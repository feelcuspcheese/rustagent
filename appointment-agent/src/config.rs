/*
 * Version: 0.1.2
 * Description: Configuration structures using the serde_yml fork.
 */

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::fs;
use anyhow::Result;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Alert,
    Booking,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MuseumConfig {
    pub name: String,
    pub slug: String,
    pub museumid: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LoginFormConfig {
    pub usernamefield: String,
    pub passwordfield: String,
    pub submitbutton: String,
    #[serde(default)]
    pub csrfselector: String,
    pub authidselector: String,
    pub loginurlselector: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BookingFormConfig {
    #[serde(default)]
    pub actionurl: String,
    pub emailfield: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SiteConfig {
    pub name: String,
    pub baseurl: String,
    pub availabilityendpoint: String,
    pub digital: bool,
    pub physical: bool,
    pub location: String,
    pub bookinglinkselector: String,
    pub successindicator: String,
    pub loginform: LoginFormConfig,
    pub bookingform: BookingFormConfig,
    pub museums: HashMap<String, MuseumConfig>,
    pub preferredslug: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CredentialConfig {
    pub name: String,
    pub username: String,
    pub password: String,
    pub email: String,
    pub site: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub active_site: String,
    pub mode: Mode,
    pub preferred_days: Vec<String>,
    pub strike_time: String,
    #[serde(with = "humantime_serde")]
    pub check_window: Duration,
    #[serde(with = "humantime_serde")]
    pub check_interval: Duration,
    #[serde(with = "humantime_serde")]
    pub pre_warm_offset: Duration,
    #[serde(with = "humantime_serde")]
    pub request_jitter: Duration,
    pub months_to_check: u32,
    pub rest_cycle_checks: u32,
    #[serde(with = "humantime_serde")]
    pub rest_cycle_duration: Duration,
    pub ntfy_topic: String,
    pub credentials: HashMap<String, CredentialConfig>,
    pub selected_credential: String,
    pub sites: HashMap<String, SiteConfig>,
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = serde_yml::from_str(&content)?;
        Ok(config)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_yml::to_string(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn get_active_site(&self) -> Option<&SiteConfig> {
        self.sites.get(&self.active_site)
    }

    pub fn get_selected_credential(&self) -> Option<&CredentialConfig> {
        self.credentials.get(&self.selected_credential)
    }
}

mod humantime_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where D: Deserializer<'de> {
        let s = String::deserialize(deserializer)?;
        humantime::parse_duration(&s).map_err(serde::de::Error::custom)
    }

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        humantime::format_duration(*duration).to_string().serialize(serializer)
    }
}
