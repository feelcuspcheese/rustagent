use anyhow::Result;
use chrono::{DateTime, Duration, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub active_site: String,
    pub mode: Mode,
    pub preferred_days: Vec<String>,
    pub strike_time: String,
    pub check_window: humantime::Duration,
    pub check_interval: humantime::Duration,
    pub pre_warm_offset: humantime::Duration,
    pub request_jitter: humantime::Duration,
    pub months_to_check: u32,
    pub rest_cycle_checks: u32,
    pub rest_cycle_duration: humantime::Duration,
    pub ntfy_topic: String,
    pub credentials: HashMap<String, Credential>,
    pub selected_credential: String,
    pub sites: HashMap<String, Site>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Alert,
    Booking,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    pub name: String,
    pub username: String,
    pub password: String,
    pub email: String,
    pub site: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Site {
    pub name: String,
    pub baseurl: String,
    pub availabilityendpoint: String,
    pub digital: bool,
    pub physical: bool,
    pub location: String,
    pub bookinglinkselector: String,
    pub successindicator: String,
    pub loginform: LoginForm,
    pub bookingform: BookingForm,
    pub museums: HashMap<String, Museum>,
    pub preferredslug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginForm {
    pub usernamefield: String,
    pub passwordfield: String,
    pub submitbutton: String,
    #[serde(default)]
    pub csrfselector: String,
    #[serde(default)]
    pub authidselector: String,
    #[serde(default)]
    pub loginurlselector: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookingForm {
    #[serde(default)]
    pub actionurl: String,
    pub emailfield: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Museum {
    pub name: String,
    pub slug: String,
    pub museumid: String,
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())?;
        let config: Config = serde_yml::from_str(&content)?;
        Ok(config)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_yml::to_string(self)?;
        fs::write(path.as_ref(), content)?;
        Ok(())
    }

    pub fn get_active_site(&self) -> Option<&Site> {
        self.sites.get(&self.active_site)
    }

    pub fn get_selected_credential(&self) -> Option<&Credential> {
        self.credentials.get(&self.selected_credential)
    }

    pub fn get_preferred_museum(&self) -> Option<(&str, &Museum)> {
        if let Some(site) = self.get_active_site() {
            return site.museums.iter().find_map(|(slug, museum)| {
                if slug == &site.preferredslug {
                    Some((slug.as_str(), museum))
                } else {
                    None
                }
            });
        }
        None
    }

    pub fn calculate_drop_time(&self) -> DateTime<Local> {
        let now = Local::now();
        let parts: Vec<&str> = self.strike_time.split(':').collect();
        if parts.len() == 2 {
            if let (Ok(hour), Ok(minute)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>()) {
                let mut drop_time = now
                    .with_hour(hour as u32)
                    .and_then(|t| t.with_minute(minute as u32))
                    .unwrap_or(now);

                if drop_time <= now {
                    drop_time = drop_time + Duration::days(1);
                }
                return drop_time;
            }
        }
        now + Duration::days(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_load() {
        let yaml = r#"
active_site: spl
mode: alert
preferred_days: ["Saturday", "Sunday"]
strike_time: "09:00"
check_window: "60s"
check_interval: "2s"
pre_warm_offset: "30s"
request_jitter: "2s"
months_to_check: 2
rest_cycle_checks: 20
rest_cycle_duration: "3s"
ntfy_topic: myappointments
credentials: {}
selected_credential: ""
sites: {}
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.active_site, "spl");
        assert_eq!(config.mode, Mode::Alert);
    }
}
