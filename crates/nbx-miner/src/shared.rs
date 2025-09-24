use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::future::Future;
use std::io::{self, Cursor};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use ibig::UBig;
use metrics::gauge;
use nbx_jetpack::log::*;
use nockapp::noun::slab::{slab_equality, NounSlab};
use nockapp::wire::Wire;
use nockvm::noun::{Noun, D, T};
use nockvm_macros::tas;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::server::WebPkiClientVerifier;
use rustls::{ClientConfig, RootCertStore, ServerConfig};
use rustls_pemfile::{certs, ec_private_keys};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::{client, server, TlsAcceptor, TlsConnector};
use uuid::Uuid;
use x509_parser::oid_registry::OID_X509_COMMON_NAME;
use x509_parser::prelude::*;
use zkvm_jetpack::form::{Belt, PRIME};
use zkvm_jetpack::noun::noun_ext::NounExt;

use crate::device::DeviceInfo;
use crate::proto::name_valid;

#[derive(Debug)]
pub struct MiningData {
    pub block_header: NounSlab,
    pub version: NounSlab,
    pub target: NounSlab,
    pub pow_len: u64,
    pub block_height: u64,
    pub fixed_nonce_atoms: Vec<Belt>,
}

impl PartialEq for MiningData {
    fn eq(&self, other: &Self) -> bool {
        if self.pow_len != other.pow_len {
            return false;
        }
        if !slab_equality(&self.block_header, &other.block_header) {
            return false;
        }
        if !slab_equality(&self.version, &other.version) {
            return false;
        }
        if !slab_equality(&self.target, &other.target) {
            return false;
        }
        true
    }
}

#[derive(Clone, Debug)]
pub struct MiningResult {
    pub miner_id: usize,
    pub attempt_millis: u32,
    pub gpu_enqueue_millis: u32,
    pub gpu_submit_millis: u32,
    pub gpu_wait_millis: u32,
    pub target_hit: bool,
    pub poke: Option<NounSlab>,
    pub effect: Option<NounSlab>,
}

pub enum MiningWire {
    Mined,
    Candidate,
    SetPubKey,
    Enable,
}

impl MiningWire {
    pub fn verb(&self) -> &'static str {
        match self {
            MiningWire::Mined => "mined",
            MiningWire::SetPubKey => "setpubkey",
            MiningWire::Candidate => "candidate",
            MiningWire::Enable => "enable",
        }
    }
}

impl Wire for MiningWire {
    const VERSION: u64 = 1;
    const SOURCE: &'static str = "miner";

    fn to_wire(&self) -> nockapp::wire::WireRepr {
        let tags = vec![self.verb().into()];
        nockapp::wire::WireRepr::new(MiningWire::SOURCE, MiningWire::VERSION, tags)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: uuid::Uuid,
    pub exp: Option<u64>,
    #[serde(default)]
    pub non_share_proofs: bool,
    #[serde(default)]
    pub telemetry: bool,
    #[serde(default)]
    pub telemetry_metrics: bool,
    #[serde(default)]
    pub max_conns_override: Option<usize>,
}

#[derive(Clone)]
pub enum Telemetry {
    Proofrate {
        machines: BTreeMap<Arc<str>, u32>,
    },
    HwInfo {
        machines: BTreeMap<Arc<str>, DeviceInfo>,
    },
}

fn make_root_store(pem_buf: &[u8]) -> RootCertStore {
    let mut store = RootCertStore::empty();
    let mut reader = Cursor::new(pem_buf);
    let certs = certs(&mut reader)
        .into_iter()
        .map(|v| v.expect("failed to parse certs"))
        .collect::<Vec<_>>();
    store.add_parsable_certificates(certs);
    store
}

fn load_cert_chain<'a>(
    cert_pem: &'a [u8],
    key_pem: &'a [u8],
) -> (Vec<CertificateDer<'a>>, PrivateKeyDer<'a>) {
    let mut r = Cursor::new(cert_pem);
    let cert_chain = certs(&mut r)
        .into_iter()
        .map(|v| v.expect("bad client cert"))
        .collect();

    let mut r = Cursor::new(key_pem);
    let mut keys = ec_private_keys(&mut r);
    let key = keys.next().unwrap().expect("bad client key");
    (cert_chain, PrivateKeyDer::Sec1(key))
}

pub struct TlsClientConfig {
    pinned_server_chain_pem: Option<&'static [u8]>,
    forced_server_name: Option<String>,
}

impl TlsClientConfig {
    pub fn pinned_default() -> Self {
        Self {
            pinned_server_chain_pem: Some(include_bytes!("../tls/server_chain.pem")),
            forced_server_name: None,
        }
    }

    pub fn forced_server_name(server_name: String) -> Self {
        Self {
            pinned_server_chain_pem: None,
            forced_server_name: Some(server_name),
        }
    }

    pub fn standard() -> Self {
        Self {
            pinned_server_chain_pem: None,
            forced_server_name: None,
        }
    }
}

pub async fn tls_connect(
    tcp: TcpStream,
    TlsClientConfig {
        forced_server_name,
        pinned_server_chain_pem,
    }: &TlsClientConfig,
    server_name: Option<String>,
) -> std::io::Result<client::TlsStream<TcpStream>> {
    let root_certs = if let Some(pinned_server_chain_pem) = pinned_server_chain_pem {
        make_root_store(pinned_server_chain_pem)
    } else {
        RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.into(),
        }
    };

    let server_name = if let Some(server_name) = forced_server_name.clone().or(server_name) {
        ServerName::try_from(server_name).map_err(|_| io::ErrorKind::InvalidInput)?
    } else {
        ServerName::IpAddress(Ipv4Addr::UNSPECIFIED.into())
    };

    let config = ClientConfig::builder()
        .with_root_certificates(root_certs)
        .with_no_client_auth();

    let connector = TlsConnector::from(Arc::new(config));

    trace!("TLS connector built");

    connector.connect(server_name, tcp).await
}

pub trait AsyncReadWrite: AsyncRead + AsyncWrite {}
impl<T: AsyncRead + AsyncWrite> AsyncReadWrite for T {}

pub struct TlsServerConfig {
    cert_chain: Vec<CertificateDer<'static>>,
    priv_key: PrivateKeyDer<'static>,
}

impl TlsServerConfig {
    /// Loads TLS server config from a file.
    pub async fn from_path(key: impl AsRef<Path>, chain: impl AsRef<Path>) -> io::Result<Self> {
        let server_chain_pem = tokio::fs::read(chain).await?;
        let server_key_pem = tokio::fs::read(key).await?;
        let (cert_chain, priv_key) =
            load_cert_chain(server_chain_pem.as_ref(), server_key_pem.as_ref());
        Ok(Self {
            cert_chain: cert_chain
                .into_iter()
                .map(CertificateDer::into_owned)
                .collect(),
            priv_key: priv_key.clone_key(),
        })
    }
}

impl Default for TlsServerConfig {
    fn default() -> Self {
        let server_chain_pem = include_bytes!("../tls/server_chain.pem");
        let server_key_pem = include_bytes!("../tls/server.key");
        let (cert_chain, priv_key) =
            load_cert_chain(server_chain_pem.as_ref(), server_key_pem.as_ref());
        Self {
            cert_chain,
            priv_key,
        }
    }
}

pub async fn tls_accept(
    tcp: TcpStream,
    TlsServerConfig {
        cert_chain,
        priv_key,
    }: &TlsServerConfig,
) -> std::io::Result<server::TlsStream<TcpStream>> {
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain.clone(), priv_key.clone_key())
        .unwrap();

    let acceptor = TlsAcceptor::from(Arc::new(config));

    trace!("TLS acceptor built");

    acceptor.accept(tcp).await
}

pub struct TimeWriter(Arc<OnceLock<Instant>>);

impl Drop for TimeWriter {
    fn drop(&mut self) {
        let _ = self.0.set(Instant::now());
    }
}

impl TimeWriter {
    pub fn new() -> (Self, Arc<OnceLock<Instant>>) {
        let arc: Arc<OnceLock<Instant>> = Arc::default();
        (Self(arc.clone()), arc)
    }
}

pub fn max_target() -> UBig {
    let p = UBig::from(PRIME);
    let p1: UBig = p.clone() - 1;
    let mut max_target = p1.clone();
    for i in 1..=4 {
        max_target += p1.clone() * p.pow(i);
    }
    max_target
}

pub fn target_to_difficulty(target: UBig) -> UBig {
    max_target() / target
}

pub fn difficulty_to_target(diff: u64) -> UBig {
    max_target() / diff
}

pub fn parse_bn(mut n: Noun) -> UBig {
    let mut cnt = 0;
    let mut val = UBig::default();

    while let Ok(c) = n.as_cell() {
        let Ok(h) = c.head().as_atom().and_then(|v| v.as_direct()) else {
            // TODO: throw error?
            break;
        };
        let v = h.data();
        // skip the %bn tag
        if cnt > 0 {
            let v = UBig::from(v);
            let v2 = v.clone() << (32 * (cnt - 1));
            val += v2;
        }
        cnt += 1;
        n = c.tail();
    }

    val
}

pub fn digest_to_target(v: Noun) -> UBig {
    let mut mul = UBig::from(1u32);
    let mut res = UBig::from(0u32);
    for v in v.uncell::<5>().expect("Expected 5 elements") {
        let v = v.as_atom().unwrap().as_u64().unwrap();
        res += (&mul) * v;
        mul *= PRIME;
    }
    res
}

pub fn to_bn(mut v: UBig) -> NounSlab {
    let mut ints = vec![D(tas!(b"bn"))];
    let zero = UBig::from(0u32);
    while v != zero {
        let int = u32::try_from(&v & !0u32).unwrap();
        ints.push(D(int as u64));
        v >>= 32;
    }
    ints.push(D(0));
    let mut slab = NounSlab::new();
    let bn = T(&mut slab, &ints);
    slab.copy_into(bn);
    slab
}

pub struct TargetMetrics {
    previous: Option<Box<Self>>,
    measurements: VecDeque<(Instant, UBig)>,
    interval: Duration,
    last_commit: Instant,
    mode: &'static str,
    level: &'static str,
}

impl TargetMetrics {
    pub fn new(interval: Duration, mode: &'static str, level: &'static str) -> Self {
        Self {
            previous: None,
            measurements: Default::default(),
            last_commit: Instant::now(),
            interval,
            mode,
            level,
        }
    }

    pub fn with_previous(self, previous: impl Into<Box<Self>>) -> Self {
        Self {
            previous: Some(previous.into()),
            ..self
        }
    }

    pub fn emit(&self) {
        let diff = self
            .get_min()
            .cloned()
            .map(target_to_difficulty)
            .unwrap_or_default();
        crate::metrics::gauge!("nbx_miner_observed_digest_hit_difficulty", "mode" => self.mode, "level" => self.level).set(diff.to_f64());
    }

    pub fn get_min(&self) -> Option<&UBig> {
        self.measurements.iter().map(|(_, v)| v).min()
    }

    pub fn measure_down(&mut self) {
        self.emit();
        let now = Instant::now();
        self.measurements
            .retain(|(v, _)| now.duration_since(*v) <= self.interval);
        let min = self.get_min().cloned();
        let delta = now.duration_since(self.last_commit);
        if delta >= self.interval {
            if delta >= 2 * self.interval {
                self.last_commit = now;
            } else {
                self.last_commit += self.interval;
            };
            if let Some(p) = self.previous.as_mut().map(|v| v.as_mut()) {
                if let Some(min) = min {
                    p.measure(min);
                } else {
                    p.measure_down();
                }
            }
        }
    }

    pub fn measure(&mut self, target: UBig) {
        let now = Instant::now();
        self.measurements.push_back((now, target));
        self.measure_down();
    }

    pub fn get_rate_statistics(&self, out: &mut Vec<String>) {
        let Some(last) = self.measurements.front() else {
            return;
        };
        let rate = (self.measurements.len() as f64) / last.0.elapsed().as_secs_f64();
        out.push(format!("{} avg: {:.02}p/s", self.level, rate));
    }
}

#[derive(Default, Debug)]
struct ConnTrackInner {
    // map(sub, set(hwid))
    conns: BTreeMap<Uuid, BTreeSet<Arc<str>>>,
}

#[derive(Clone, Default, Debug)]
pub struct ConnTrack(Arc<Mutex<ConnTrackInner>>);

impl ConnTrack {
    pub fn connect(&self, sub: Uuid, hwid: Arc<str>, max_sub_conns: usize) -> Option<ConnHandle> {
        let mut track = self.0.lock().unwrap();
        let mut sub_entry = track.conns.entry(sub.clone()).or_default();

        // Within limit and not connected
        if sub_entry.len() < max_sub_conns && sub_entry.insert(hwid.clone()) {
            Some(ConnHandle {
                track: self.clone(),
                sub,
                hwid,
            })
        } else {
            None
        }
    }

    pub fn emit_metrics(&self) -> usize {
        let track = self.0.lock().unwrap();
        gauge!("nbx_miner_conntrack_subs").set(track.conns.len() as f64);
        let mut total = 0;
        for (s, v) in &track.conns {
            gauge!("nbx_miner_conntrack_sub_active", "sub" => s.to_string()).set(v.len() as f64);
            total += v.len();
        }
        gauge!("nbx_miner_conntrack_active").set(total as f64);
        total
    }
}

#[derive(Debug)]
pub struct ConnHandle {
    track: ConnTrack,
    sub: Uuid,
    hwid: Arc<str>,
}

impl Drop for ConnHandle {
    fn drop(&mut self) {
        let mut track = self.track.0.lock().unwrap();
        let Entry::Occupied(mut sub) = track.conns.entry(self.sub.clone()) else {
            panic!("Not occupied when expected occupied");
        };
        let sub_value = sub.get_mut();
        assert!(sub_value.remove(&self.hwid));
        if sub_value.is_empty() {
            sub.remove();
        }
    }
}
