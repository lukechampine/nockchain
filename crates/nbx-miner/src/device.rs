use std::net::SocketAddr;
use std::sync::Arc;

use bincode::{Decode, Encode};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use sysinfo::{MemoryRefreshKind, System};

use crate::proto::NAME_MAX_LENGTH;

const LOG_TARGET: &str = "nbx::device";

macro_rules! cfg_feature {
    ($feature:expr) => {
        cfg!(target_feature = $feature)
    };
}

#[cfg(target_arch = "aarch64")]
macro_rules! cpu_arch {
    ($feature:ident) => {
        "aarch64"
    };
}

#[cfg(target_arch = "x86_64")]
macro_rules! cpu_arch {
    ($feature:ident) => {{
        let mut cpu_level = "x86_64";

        if !$feature!("popcnt")
            || !$feature!("sse3")
            || !$feature!("sse4.1")
            || !$feature!("sse4.2")
        {
            return cpu_level;
        }
        cpu_level = "x86_64-v2";

        if !$feature!("avx")
            || !$feature!("avx2")
            || !$feature!("bmi1")
            || !$feature!("bmi2")
            || !$feature!("f16c")
            || !$feature!("fma")
            || !$feature!("lzcnt")
            || !$feature!("movbe")
            || !$feature!("xsave")
        {
            return cpu_level;
        }
        cpu_level = "x86_64-v3";

        if !$feature!("avx512f")
            || !$feature!("avx512cd")
            || !$feature!("avx512dq")
            || !$feature!("avx512bw")
            || !$feature!("avx512vl")
        {
            return cpu_level;
        }
        cpu_level = "x86_64-v4";

        cpu_level
    }};
}

#[cfg(not(target_arch = "x86_64"))]
pub fn runtime_cpu_level() -> &'static str {
    cpu_level()
}

#[cfg(target_arch = "x86_64")]
pub fn runtime_cpu_level() -> &'static str {
    cpu_arch!(is_x86_feature_detected)
}

pub const fn cpu_level() -> &'static str {
    cpu_arch!(cfg_feature)
}

// NOTE: changing this or any of the children requires change in the nbx protocol version.
#[derive(Clone, Debug, Encode, Decode, Serialize, Deserialize)]
pub struct DeviceInfoWithSockets {
    #[serde(flatten)]
    pub device: DeviceInfo,
    pub sockets_outgoing: Vec<SocketAddr>,
    pub sockets_incoming: Vec<SocketAddr>,
}

#[derive(Clone, Debug, Encode, Decode, Serialize, Deserialize)]
pub struct GpuInfo {}

#[derive(Clone, Debug, Encode, Decode, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub binary_version: String,
    pub binary_arch: String,
    pub is_proxy: bool,
    pub client_name: Option<String>,
    pub hostname: Option<String>,
    pub os_version: String,
    pub cpu_models: Vec<String>,
    pub cpu_count: u64,
    pub cpu_arch: String,
    pub ram_mb: u64,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub gpus: Vec<GpuInfo>,
}

impl DeviceInfo {
    pub fn new(client_name: Option<String>, is_proxy: bool, sys: &System) -> Self {
        let hostname = System::host_name();
        let os_version = System::long_os_version().unwrap_or_else(|| "Unknown".to_string());
        let cpu_models = sys
            .cpus()
            .iter()
            .map(|v| v.brand().to_string())
            .unique()
            .collect::<Vec<_>>();
        let ram_mb = sys.total_memory() / (1024 * 1024);

        Self {
            binary_version: env!("CARGO_PKG_VERSION").to_string(),
            binary_arch: cpu_level().to_string(),
            is_proxy,
            client_name,
            hostname,
            os_version,
            cpu_models,
            cpu_count: sys.cpus().len() as u64,
            cpu_arch: runtime_cpu_level().to_string(),
            ram_mb,
            gpus: vec![],
        }
    }

    pub fn with_outgoing_socket(&self, socket: SocketAddr) -> DeviceInfoWithSockets {
        DeviceInfoWithSockets {
            device: self.clone(),
            sockets_outgoing: vec![socket],
            sockets_incoming: vec![],
        }
    }
}

#[derive(Clone, Debug)]
pub struct Device {
    pub hwid: Arc<str>,
    pub info: DeviceInfo,
}

impl Device {
    pub fn new(client_name: Option<String>, is_proxy: bool) -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory_specifics(MemoryRefreshKind::default().with_ram());

        let info = DeviceInfo::new(client_name.clone(), is_proxy, &sys);

        let key = client_name.as_deref().unwrap_or("hwid");
        let hwid: Arc<str> = hwid(&format!("{is_proxy}-{key}"), &info);

        crate::log!(
            trace, "Device info: cpu_arch = {}; cpu_count = {}; ram_mb = {}; hwid = {hwid}",
            info.cpu_arch, info.cpu_count, info.ram_mb
        );
        if info.cpu_arch != info.binary_arch {
            crate::log!(
                warn, "Binary architecture ({}) does not match running CPU architecture ({}). Performance or stability may be degraded.",
                info.binary_arch, info.cpu_arch
            );
        }

        Self { hwid, info }
    }
}

#[cfg(target_os = "linux")]
fn machine_id() -> Option<String> {
    for p in ["/var/lib/dbus/machine-id", "/etc/machine-id"] {
        if let Ok(d) = std::fs::read_to_string(p) {
            return Some(d);
        }
    }
    None
}

fn hwid(key: &str, dev: &DeviceInfo) -> Arc<str> {
    let mut state = Sha3_256::new();
    state.update(&key);
    state.update(&machine_id().unwrap());
    state.update(&dev.os_version);
    for cpu in &dev.cpu_models {
        state.update(cpu);
    }
    let mut ret = format!("{:x}", state.finalize());
    ret.truncate(NAME_MAX_LENGTH);
    ret.into()
}
