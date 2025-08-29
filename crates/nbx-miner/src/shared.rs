use std::collections::BTreeMap;
use std::future::Future;
use std::io::{self, Cursor};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, OnceLock};
use std::pin::Pin;
use std::time::Instant;

use nockapp::noun::slab::{slab_equality, NounSlab};
use nockapp::wire::Wire;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::server::WebPkiClientVerifier;
use rustls::{ClientConfig, RootCertStore, ServerConfig};
use rustls_pemfile::{certs, ec_private_keys, rsa_private_keys};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::client;
use tokio_rustls::server;
use tokio_rustls::{TlsAcceptor, TlsConnector};
use nbx_jetpack::log::*;
use x509_parser::oid_registry::OID_X509_COMMON_NAME;
use x509_parser::prelude::*;
use zkvm_jetpack::form::Belt;

use crate::proto::{name_valid, NAME_MAX_LENGTH};

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

fn load_cert_chain<'a>(cert_pem: &'a [u8], key_pem: &'a [u8]) -> (Vec<CertificateDer<'a>>, PrivateKeyDer<'a>) {
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

#[derive(Clone, Copy)]
pub struct TlsClientConfig {
    server_chain_pem: &'static [u8],
    client_chain_pem: &'static [u8],
    client_key_pem: &'static [u8]
}

impl Default for TlsClientConfig {
    fn default() -> Self {
        Self {
            server_chain_pem: include_bytes!("../tls/server_chain.pem"),
            client_chain_pem: include_bytes!("../tls/client_chain.pem"),
            client_key_pem: include_bytes!("../tls/client.key"),
        }
    }
}

pub async fn tls_connect(
    tcp: TcpStream,
    TlsClientConfig {
        server_chain_pem,
        client_chain_pem,
        client_key_pem
    }: TlsClientConfig,
) -> std::io::Result<client::TlsStream<TcpStream>> {
    let (cert_chain, priv_key) = load_cert_chain(client_chain_pem, client_key_pem);

    let config = ClientConfig::builder()
        .with_root_certificates(make_root_store(server_chain_pem))
        .with_client_auth_cert(cert_chain, priv_key)
        .expect("invalid client auth setup");

    let connector = TlsConnector::from(Arc::new(config));

    trace!("TLS connector built");

    connector
        .connect(ServerName::IpAddress(Ipv4Addr::UNSPECIFIED.into()), tcp)
        //.connect("nock.box".try_into().unwrap(), tcp)
        .await
}

pub trait AsyncReadWrite: AsyncRead + AsyncWrite {}
impl<T: AsyncRead + AsyncWrite> AsyncReadWrite for T {}

pub async fn tls_connect_wrap(
    tcp: impl Future<Output = std::io::Result<TcpStream>>,
    tls: Option<TlsClientConfig>
) -> std::io::Result<Pin<Box<dyn 'static + AsyncReadWrite + Send>>> {
    let tcp = tcp.await?;
    trace!("Connected");
    if let Some(tls) = tls {
        let tls = tls_connect(tcp, tls).await?;
        Ok(Box::pin(tls))
    } else {
        Ok(Box::pin(tcp))
    }
}

#[derive(Clone, Copy)]
pub struct TlsServerConfig {
    server_chain_pem: &'static [u8],
    server_key_pem: &'static [u8],
    ca_pem: &'static [u8],
}

impl Default for TlsServerConfig {
    fn default() -> Self {
        Self {
            server_chain_pem: include_bytes!("../tls/server_chain.pem"),
            server_key_pem: include_bytes!("../tls/server.key"),
            ca_pem: include_bytes!("../tls/ca.pem"),
        }
    }
}

pub async fn tls_accept(
    tcp: TcpStream,
    TlsServerConfig {
        server_chain_pem,
        server_key_pem,
        ca_pem
    }: TlsServerConfig,
) -> std::io::Result<server::TlsStream<TcpStream>> {
    let (cert_chain, priv_key) = load_cert_chain(server_chain_pem, server_key_pem);

    let roots = make_root_store(ca_pem);

    let verifier = WebPkiClientVerifier::builder(roots.into())
        .build()
        .expect("Unable to build verifier");

    let config = ServerConfig::builder()
        .with_client_cert_verifier(verifier)
        .with_single_cert(cert_chain, priv_key)
        .unwrap();

    let acceptor = TlsAcceptor::from(Arc::new(config));

    trace!("TLS acceptor built");

    acceptor
        .accept(tcp)
        .await
}

pub async fn tls_accept_wrap(
    tcp: impl Future<Output = std::io::Result<(TcpStream, SocketAddr)>>,
    tls: Option<TlsServerConfig>
) -> std::io::Result<(Pin<Box<dyn 'static + AsyncReadWrite + Send>>, SocketAddr, Arc<str>)> {
    let (tcp, addr) = tcp.await?;
    trace!("Accepted {addr}");
    if let Some(tls) = tls {
        let tls = tls_accept(tcp, tls).await?;
        let Some(tls_name) = tls.get_ref().1.peer_certificates().and_then(|v| {
            let c = v.first()?;
            let (_rem, x509) = X509Certificate::from_der(c.as_ref()).ok()?;
            let cn = x509.subject()
                .iter_attributes()
                .find(|attr| attr.attr_type() == &OID_X509_COMMON_NAME)
                .and_then(|attr| attr.as_str().ok())?;
            if !name_valid(cn) {
                error!("cn={cn} with too long of a name");
                None
            } else {
                Some(cn.into())
            }
        }) else {
            return Err(io::ErrorKind::InvalidData.into());
        };
        Ok((Box::pin(tls), addr, tls_name))
    } else {
        Ok((Box::pin(tcp), addr, "__notls".into()))
    }
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
