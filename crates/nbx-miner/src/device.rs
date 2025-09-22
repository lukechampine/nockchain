use std::sync::Arc;

use bincode::{Decode, Encode};
use machineid_rs::{Encryption, HWIDComponent, IdBuilder};
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
    pub fn new(client_name: Option<String>, is_proxy: bool) -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory_specifics(MemoryRefreshKind::default().with_ram());

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
        let key = client_name.as_deref().unwrap_or("hwid");
        let hwid: Arc<str> = IdBuilder::new(Encryption::MD5)
            .add_component(HWIDComponent::SystemID)
            .add_component(HWIDComponent::CPUID)
            .add_component(HWIDComponent::CPUCores)
            .add_component(HWIDComponent::OSName)
            .build(&format!("{is_proxy}-{key}"))
            .expect("Unable to gather machine info")
            .split_off(NAME_MAX_LENGTH)
            .into();

        let info = DeviceInfo::new(client_name, is_proxy);

        Self { hwid, info }
    }
}
