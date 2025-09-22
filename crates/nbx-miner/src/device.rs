use std::sync::Arc;

use bincode::{Decode, Encode};
use sha3::{Digest, Sha3_256};
use sysinfo::{MemoryRefreshKind, System};

use crate::proto::NAME_MAX_LENGTH;

#[derive(Clone, Debug, Encode, Decode)]
pub struct DeviceInfo {
    pub is_proxy: bool,
    pub client_name: Option<String>,
    pub hostname: Option<String>,
    pub os_version: String,
    pub cpu_models: Vec<String>,
    pub ram_mb: u64,
}

impl DeviceInfo {
    pub fn new(client_name: Option<String>, is_proxy: bool, sys: &System) -> Self {
        let hostname = System::host_name();
        let os_version = System::long_os_version().unwrap_or_else(|| "Unknown".to_string());
        let cpu_models = sys
            .cpus()
            .iter()
            .map(|v| v.brand().to_string())
            .collect::<Vec<_>>();
        let ram_mb = sys.total_memory() / (1024 * 1024);

        Self {
            is_proxy,
            client_name,
            hostname,
            os_version,
            cpu_models,
            ram_mb,
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
