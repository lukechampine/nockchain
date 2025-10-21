use std::net::IpAddr;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lru::LruCache;
use metrics::counter;
use nbx_jetpack::log::{debug, error, info};
use serde::{Deserialize, Serialize};
use tracing::warn;
use uuid::Uuid;

use crate::compliance::ip_address_checks::{IpAddressCheckDecision, IpAddressCheckReason};
use crate::db::DatabaseHandle;

const IPDATA_TIMEOUT: Duration = Duration::from_secs(5);
const IPDATA_API_URL: &str = "https://api.ipdata.co";

// Blocked countries due to U.S. sanctions and prohibitions
const BLOCKED_COUNTRIES: &[&str] = &[
    "RU", // Russia - U.S. EO 14071 service prohibitions
    "UA", // Ukraine - entire country
    "CU", // Cuba - OFAC country program
    "IR", // Iran - OFAC country program
    "KP", // North Korea (DPRK) - OFAC country program
    "SY", // Syria - OFAC country program
];

// Blocked U.S. states due to licensing/regulatory risk
const BLOCKED_US_STATES: &[&str] = &[
    "NY", // New York
    "WA", // Washington
    "CT", // Connecticut
    "LA", // Louisiana
];

#[derive(Debug, Deserialize, Serialize)]
struct IpDataResponse {
    ip: String,

    is_eu: bool,
    city: Option<String>,
    region: Option<String>,
    region_code: Option<String>,
    region_type: Option<String>,
    country_name: String,
    country_code: String,
    continent_name: String,
    continent_code: String,
    latitude: f64,
    longitude: f64,
    postal: Option<String>,

    threat: ThreatInfo,
}

#[derive(Debug, Deserialize, Serialize)]
struct ThreatInfo {
    is_tor: bool,
    is_icloud_relay: bool,
    is_proxy: bool,
    is_datacenter: bool,
    is_anonymous: bool,
    is_bogon: bool,
    is_known_attacker: bool,
    is_known_abuser: bool,
}

#[derive(Clone)]
pub struct IpAddressChecker {
    api_key: String,
    client: reqwest::Client,
    lru: Arc<Mutex<LruCache<(Uuid, IpAddr), IpAddressCheckDecision>>>,
}

impl IpAddressChecker {
    pub fn from_env() -> Self {
        Self::new(std::env::var("NBX_IP_DATA_API_TOKEN").unwrap())
    }

    pub fn new(api_key: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(IPDATA_TIMEOUT)
            .build()
            .expect("Failed to create HTTP client");
        let lru = Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(16384).unwrap())));

        Self {
            api_key,
            client,
            lru,
        }
    }

    async fn check_with_ipdata(&self, ip: IpAddr) -> Result<IpDataResponse, reqwest::Error> {
        let url = format!("{IPDATA_API_URL}/{ip}?api-key={}", self.api_key);

        let response = self.client.get(&url).send().await?;
        response.json::<IpDataResponse>().await
    }

    pub fn evaluate_response(&self, response: &IpDataResponse) -> IpAddressCheckDecision {
        // Check for blocked countries (compliance requirement)
        let country_upper = response.country_code.to_uppercase();
        if BLOCKED_COUNTRIES.contains(&country_upper.as_str()) {
            info!(
                "Blocking connection from sanctioned country: {} ({})",
                country_upper, response.country_name
            );
            return IpAddressCheckDecision::Block(IpAddressCheckReason::Country);
        }

        // Check for blocked U.S. states (if country is US)
        if country_upper == "US" {
            let Some(region_code) = response.region_code.as_ref() else {
                info!("Blocking connection from unknown U.S. state",);
                return IpAddressCheckDecision::Block(IpAddressCheckReason::Country);
            };

            let region_upper = region_code.to_uppercase();
            if BLOCKED_US_STATES.contains(&region_upper.as_str()) {
                info!(
                    "Blocking connection from restricted U.S. state: {} ({})",
                    region_upper, region_code
                );
                return IpAddressCheckDecision::Block(IpAddressCheckReason::Country);
            }
        }

        // Check for Proxy or TOR connection
        if response.threat.is_proxy || response.threat.is_tor {
            info!("Blocking anonymous connection");
            return IpAddressCheckDecision::Block(IpAddressCheckReason::Vpn);
        }

        IpAddressCheckDecision::Allow
    }

    fn lru_ip_address_details(&self, sub: Uuid, ip: IpAddr) -> Option<IpAddressCheckDecision> {
        let mut lru = self.lru.lock().unwrap();
        lru.get(&(sub, ip)).cloned()
    }

    fn lru_insert_ip_address_details(
        &self,
        sub: Uuid,
        ip: IpAddr,
        decision: IpAddressCheckDecision,
    ) {
        let mut lru = self.lru.lock().unwrap();
        lru.push((sub, ip), decision);
    }

    pub async fn check_ip(
        &self,
        ip: IpAddr,
        sub: Uuid,
        db: &DatabaseHandle,
    ) -> Option<IpAddressCheckDecision> {
        if ip.is_loopback() {
            return Some(IpAddressCheckDecision::Allow);
        }

        // Check cache first
        if let Some(cached_decision) = self.lru_ip_address_details(sub, ip) {
            debug!("Using cached IP decision for {ip}: {:?}", cached_decision);
            counter!("nbx_miner_ip_check_lru_cache_hit_total").increment(1);
            return Some(cached_decision);
        }
        counter!("nbx_miner_ip_check_lru_cache_miss_total").increment(1);

        // Check db then
        if let Some(cached_decision) = db.check_ip_address_details(sub, ip).await {
            debug!("Using cached IP decision for {ip}: {:?}", cached_decision);
            counter!("nbx_miner_ip_check_cache_hit_total").increment(1);
            self.lru_insert_ip_address_details(sub, ip, cached_decision.clone());
            return Some(cached_decision);
        }
        counter!("nbx_miner_ip_check_cache_miss_total").increment(1);

        // Perform the check
        match self.check_with_ipdata(ip).await {
            Ok(response) => {
                let decision = self.evaluate_response(&response);

                // Store result in database
                let response_json = serde_json::to_string(&response).unwrap_or_default();
                db.submit_ip_address_details(sub, ip, decision, response_json);

                counter!(
                    "nbx_miner_ip_check_decision_total",
                    "decision" => decision.decision_as_str().to_string()
                )
                .increment(1);

                self.lru_insert_ip_address_details(sub, ip, decision.clone());
                Some(decision)
            }
            Err(e) => {
                error!("Failed to check IP {ip} with ipdata.co: {e}");
                counter!("nbx_miner_ip_check_error_total").increment(1);
                None
            }
        }
    }
}
