use anyhow::Result;
use chrono::{DateTime, Duration, Local, NaiveDate};
use dashmap::DashMap;
use rand::Rng;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::booker::{Booker, BookingPageState};
use crate::client_pool::ClientPool;
use crate::config::{Config, Mode};
use crate::scraper::{AvailabilitySlot, Scraper};

#[derive(Debug, Clone)]
pub struct AgentEvent {
    pub event: String,
    #[serde(flatten)]
    pub data: serde_json::Value,
}

impl AgentEvent {
    pub fn new(event: &str) -> Self {
        Self {
            event: event.to_string(),
            data: serde_json::json!({}),
        }
    }

    pub fn with_data(mut self, key: &str, value: serde_json::Value) -> Self {
        if let serde_json::Value::Object(ref mut map) = self.data {
            map.insert(key.to_string(), value);
        }
        self
    }
}

pub struct Agent {
    config: Arc<Config>,
    client_pool: Arc<ClientPool>,
    log_tx: broadcast::Sender<AgentEvent>,
    seen_dates: Arc<DashMap<String, ()>>,
}

impl Agent {
    pub fn new(
        config: Config,
        log_tx: broadcast::Sender<AgentEvent>,
    ) -> Result<Self> {
        let client_pool = Arc::new(ClientPool::new()?);
        
        Ok(Self {
            config: Arc::new(config),
            client_pool,
            log_tx,
            seen_dates: Arc::new(DashMap::new()),
        })
    }

    /// Emit a log event
    fn emit(&self, event: AgentEvent) {
        let _ = self.log_tx.send(event.clone());
        // Also print to stdout
        println!("{}", serde_json::to_string(&event).unwrap_or_default());
    }

    /// Run the agent
    pub async fn run(&self) -> Result<()> {
        let run_id = Uuid::new_v4().to_string();
        let drop_time = self.config.calculate_drop_time();

        info!("Starting agent run");
        self.emit(AgentEvent::new("agent_start")
            .with_data("run_id", serde_json::json!(run_id))
            .with_data("drop_time", serde_json::json!(drop_time.to_rfc3339())));

        // Pre-warm phase
        self.emit(AgentEvent::new("pre_warm_start"));
        self.pre_warm().await?;
        self.emit(AgentEvent::new("pre_warm_complete"));

        // Wait until exact drop time
        let pre_warm_deadline = drop_time - Duration::seconds(self.config.pre_warm_offset.into());
        let now = Local::now();
        
        if now < pre_warm_deadline {
            let sleep_duration = (pre_warm_deadline - now).num_milliseconds();
            if sleep_duration > 10 {
                tokio::time::sleep(std::time::Duration::from_millis(sleep_duration as u64 - 10)).await;
            }
            // Spin-loop for last 10ms
            while Local::now() < pre_warm_deadline {
                std::hint::spin_loop();
            }
        }

        // Check window loop
        let check_window_start = drop_time;
        let check_window_end = drop_time + Duration::seconds(self.config.check_window.into());
        
        self.emit(AgentEvent::new("check_window_start")
            .with_data("deadline", serde_json::json!(check_window_end.to_rfc3339())));

        let mut check_count = 0u32;
        let mut found_slots: Vec<AvailabilitySlot> = Vec::new();

        while Local::now() <= check_window_end {
            // Apply jitter before each check
            let jitter_ms = rand::thread_rng().gen_range(0..self.config.request_jitter.into()) as u64;
            tokio::time::sleep(std::time::Duration::from_millis(jitter_ms)).await;

            // Fetch availability
            match self.fetch_all_availability().await {
                Ok(slots) => {
                    for slot in slots {
                        if !self.seen_dates.contains_key(&slot.date) {
                            self.seen_dates.insert(slot.date.clone(), ());
                            self.emit(AgentEvent::new("availability_found")
                                .with_data("date", serde_json::json!(slot.date))
                                .with_data("booking_url", serde_json::json!(slot.booking_url)));
                            found_slots.push(slot);
                        }
                    }
                    
                    if found_slots.is_empty() {
                        debug!("No new availability found in this check");
                    }
                }
                Err(e) => {
                    error!("Failed to fetch availability: {}", e);
                }
            }

            check_count += 1;

            // Rest cycle
            if check_count >= self.config.rest_cycle_checks 
                && self.config.check_window.into() > 60 
            {
                debug!("Entering rest cycle");
                tokio::time::sleep(std::time::Duration::from_secs(
                    self.config.rest_cycle_duration.into()
                )).await;
                check_count = 0;
            }

            // If we found slots and mode is alert, send notification and stop
            if !found_slots.is_empty() && self.config.mode == Mode::Alert {
                self.send_notification(&found_slots).await?;
                self.emit(AgentEvent::new("agent_finished"));
                return Ok(());
            }

            // If we found slots and mode is booking, attempt booking
            if !found_slots.is_empty() && self.config.mode == Mode::Booking {
                if let Some(_success) = self.attempt_bookings(&found_slots).await? {
                    self.emit(AgentEvent::new("agent_finished"));
                    return Ok(());
                }
            }

            // Small interval between checks
            tokio::time::sleep(std::time::Duration::from_secs(
                self.config.check_interval.into()
            )).await;
        }

        self.emit(AgentEvent::new("check_window_expired"));
        self.emit(AgentEvent::new("agent_finished"));
        Ok(())
    }

    /// Pre-warm: establish TLS session and cookies
    async fn pre_warm(&self) -> Result<()> {
        if let Some(site) = self.config.get_active_site() {
            let client = self.client_pool.next();
            let _ = client.get(&site.baseurl).send().await?;
            debug!("Pre-warm complete for {}", site.baseurl);
        }
        Ok(())
    }

    /// Fetch availability for all months in range
    async fn fetch_all_availability(&self) -> Result<Vec<AvailabilitySlot>> {
        let site = self.config.get_active_site()
            .ok_or_else(|| anyhow::anyhow!("No active site configured"))?;
        
        let (_slug, museum) = self.config.get_preferred_museum()
            .ok_or_else(|| anyhow::anyhow!("No preferred museum configured"))?;

        let drop_time = self.config.calculate_drop_time();
        let mut all_slots = Vec::new();

        // Fetch for current month and next (months_to_check - 1) months
        for month_offset in 0..self.config.months_to_check {
            let target_date = drop_time.date_naive() + Duration::days((month_offset * 30) as i64);
            
            let scraper = Scraper::new(self.client_pool.next().clone());
            match scraper.fetch_availability(site, museum, target_date).await {
                Ok(slots) => {
                    all_slots.extend(slots);
                }
                Err(e) => {
                    warn!("Failed to fetch availability for month {}: {}", month_offset, e);
                    // Try relaxed parsing
                    // This would require re-fetching with relaxed parser
                }
            }
        }

        Ok(all_slots)
    }

    /// Send ntfy notification (alert mode)
    async fn send_notification(&self, slots: &[AvailabilitySlot]) -> Result<()> {
        // Sort by preferred days (weekends first)
        let mut sorted_slots: Vec<&AvailabilitySlot> = slots.iter().collect();
        sorted_slots.sort_by(|a, b| {
            let a_is_preferred = self.is_preferred_day(&a.date);
            let b_is_preferred = self.is_preferred_day(&b.date);
            b_is_preferred.cmp(&a_is_preferred)
        });

        // Take up to 3 actions
        let actions: Vec<&AvailabilitySlot> = sorted_slots.into_iter().take(3).collect();
        
        let title = format!("🎫 {} Availability Found!", 
            self.config.get_active_site().map(|s| s.name.as_str()).unwrap_or("Appointment"));
        
        let mut message = String::from("Available dates:\n");
        for slot in &actions {
            message.push_str(&format!("📅 {} - {}\n", slot.date, slot.booking_url));
        }

        // Send to ntfy
        let ntfy_url = format!("https://ntfy.sh/{}", self.config.ntfy_topic);
        let client = reqwest::Client::new();
        let _ = client.post(&ntfy_url)
            .header("Title", &title)
            .body(message)
            .send()
            .await;

        self.emit(AgentEvent::new("notification_sent")
            .with_data("title", serde_json::json!(title))
            .with_data("topic", serde_json::json!(self.config.ntfy_topic)));

        info!("Notification sent to {}", self.config.ntfy_topic);
        Ok(())
    }

    /// Attempt bookings for found slots (booking mode)
    async fn attempt_bookings(&self, slots: &[AvailabilitySlot]) -> Result<Option<bool>> {
        let site = self.config.get_active_site()
            .ok_or_else(|| anyhow::anyhow!("No active site configured"))?;
        
        let credential = self.config.get_selected_credential()
            .ok_or_else(|| anyhow::anyhow!("No selected credential configured"))?;

        // Sort by preferred days
        let mut sorted_slots: Vec<&AvailabilitySlot> = slots.iter().collect();
        sorted_slots.sort_by(|a, b| {
            let a_is_preferred = self.is_preferred_day(&a.date);
            let b_is_preferred = self.is_preferred_day(&b.date);
            b_is_preferred.cmp(&a_is_preferred)
        });

        for slot in sorted_slots {
            self.emit(AgentEvent::new("booking_attempt")
                .with_data("date", serde_json::json!(slot.date)));

            match self.book_slot(slot, site, credential).await {
                Ok(true) => {
                    self.emit(AgentEvent::new("booking_success")
                        .with_data("date", serde_json::json!(slot.date)));
                    return Ok(Some(true));
                }
                Ok(false) => {
                    self.emit(AgentEvent::new("booking_failed")
                        .with_data("date", serde_json::json!(slot.date))
                        .with_data("error", serde_json::json!("Booking returned false")));
                }
                Err(e) => {
                    self.emit(AgentEvent::new("booking_failed")
                        .with_data("date", serde_json::json!(slot.date))
                        .with_data("error", serde_json::json!(e.to_string())));
                }
            }
        }

        Ok(Some(false))
    }

    /// Book a single slot
    async fn book_slot(
        &self,
        slot: &AvailabilitySlot,
        site: &crate::config::Site,
        credential: &crate::config::Credential,
    ) -> Result<bool> {
        let booker = Booker::new(self.client_pool.next().clone());

        // Follow booking URL
        match booker.follow_booking_url(&slot.booking_url).await? {
            BookingPageState::LoginRequired(login_fields) => {
                // Perform login
                if !booker.login(&login_fields, credential).await? {
                    return Ok(false);
                }

                // After login, follow URL again to get booking form
                match booker.follow_booking_url(&slot.booking_url).await? {
                    BookingPageState::BookingReady(booking_fields) => {
                        booker.book(&booking_fields, credential, &site.successindicator).await?
                    }
                    _ => return Ok(false),
                }
            }
            BookingPageState::BookingReady(booking_fields) => {
                booker.book(&booking_fields, credential, &site.successindicator).await?
            }
            BookingPageState::Unknown(_) => return Ok(false),
        };

        Ok(true)
    }

    /// Check if a date string matches preferred days
    fn is_preferred_day(&self, date_str: &str) -> bool {
        if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            let day_name = date.format("%A").to_string();
            self.config.preferred_days.iter().any(|d| d == &day_name)
        } else {
            false
        }
    }
}

// Helper trait implementations for humantime::Duration
impl From<humantime::Duration> for i64 {
    fn from(d: humantime::Duration) -> Self {
        d.as_secs() as i64
    }
}

impl From<humantime::Duration> for u64 {
    fn from(d: humantime::Duration) -> Self {
        d.as_secs()
    }
}

impl From<humantime::Duration> for u32 {
    fn from(d: humantime::Duration) -> Self {
        d.as_secs() as u32
    }
}
