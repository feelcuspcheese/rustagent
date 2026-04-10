/*
 * Version: 0.1.5
 * Description: Orchestration engine. Fixed field naming typo.
 */

use anyhow::{anyhow, Result};
use chrono::{Duration as ChronoDuration, Local, NaiveTime};
use rand::Rng;
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::{sleep, Duration};
use tracing::info;

use crate::booker::Booker;
use crate::client_pool::ClientPool;
use crate::config::{Config, Mode};
use crate::scraper::Scraper;

pub struct Agent {
    config: Arc<Config>,
    client_pool: Arc<ClientPool>,
    run_id: String,
}

impl Agent {
    pub fn new(config: Config, client_pool: ClientPool) -> Self {
        Self {
            config: Arc::new(config),
            client_pool: Arc::new(client_pool),
            run_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    pub async fn run(&self) -> Result<()> {
        let strike_time = NaiveTime::parse_from_str(&self.config.strike_time, "%H:%M")
            .map_err(|_| anyhow!("Invalid strike_time format in config"))?;

        info!("{}", json!({
            "event": "agent_start",
            "run_id": self.run_id,
            "drop_time": self.config.strike_time,
            "mode": format!("{:?}", self.config.mode).to_lowercase()
        }));

        self.wait_for_strike(strike_time).await?;
        self.execution_loop(strike_time).await?;

        info!("{}", json!({"event": "agent_finished", "run_id": self.run_id}));
        Ok(())
    }

    async fn wait_for_strike(&self, strike_time: NaiveTime) -> Result<()> {
        let now = Local::now();
        let mut strike_dt = now.date_naive().and_time(strike_time).and_local_timezone(Local).unwrap();

        if strike_dt < now {
            strike_dt = strike_dt + ChronoDuration::days(1);
        }

        let pre_warm_at = strike_dt - ChronoDuration::from_std(self.config.pre_warm_offset)?;
        
        let time_until_prewarm = pre_warm_at.signed_duration_since(Local::now());
        if time_until_prewarm.num_milliseconds() > 0 {
            sleep(time_until_prewarm.to_std()?).await;
        }

        info!("{}", json!({"event": "pre_warm_start"}));
        let client = self.client_pool.next();
        let site = self.config.get_active_site().ok_or_else(|| anyhow!("Active site not found"))?;
        let _ = client.get(&site.baseurl).send().await; 
        info!("{}", json!({"event": "pre_warm_complete"}));

        let time_to_strike = strike_dt.signed_duration_since(Local::now());
        if time_to_strike.num_milliseconds() > 10 {
            sleep(time_to_strike.to_std()? - Duration::from_millis(10)).await;
        }

        while Local::now() < strike_dt {
            std::hint::spin_loop();
        }

        Ok(())
    }

    async fn execution_loop(&self, strike_time: NaiveTime) -> Result<()> {
        let now = Local::now();
        let strike_dt = now.date_naive().and_time(strike_time).and_local_timezone(Local).unwrap();
        let deadline = strike_dt + ChronoDuration::from_std(self.config.check_window)?;

        info!("{}", json!({"event": "check_window_start", "deadline": deadline.to_rfc3339()}));

        let mut check_count = 0;
        let mut seen_dates = HashSet::new();
        let site = self.config.get_active_site().ok_or_else(|| anyhow!("Active site not found"))?;
        
        // Fixed: Use preferredslug instead of preferred_slug
        let museum = site.museums.get(&site.preferredslug).ok_or_else(|| anyhow!("Museum not found"))?;
        let semaphore = Arc::new(Semaphore::new(3)); 

        while Local::now() < deadline {
            let jitter_ms = rand::thread_rng().gen_range(0..self.config.request_jitter.as_millis() as u64);
            sleep(Duration::from_millis(jitter_ms)).await;

            let mut tasks = Vec::new();
            let base_date = Local::now().date_naive();

            for m in 0..self.config.months_to_check {
                let month_date = base_date + ChronoDuration::days(m as i64 * 30);
                let client = self.client_pool.next();
                let sem = Arc::clone(&semaphore);
                let site_cfg = site.clone();
                let museum_id = museum.museumid.clone();

                tasks.push(tokio::spawn(async move {
                    let _permit = sem.acquire().await.unwrap();
                    let url = Scraper::build_availability_url(&site_cfg, &museum_id, month_date)?;
                    info!("{}", json!({"event": "availability_fetch", "url": url, "month": month_date.format("%Y-%m").to_string()}));
                    
                    let resp = client.get(&url).send().await?;
                    let html = resp.text().await?;
                    Scraper::parse_availability(&html, &site_cfg)
                }));
            }

            for task in tasks {
                if let Ok(Ok(availabilities)) = task.await {
                    for avail in availabilities {
                        if !seen_dates.contains(&avail.date) {
                            seen_dates.insert(avail.date);
                            info!("{}", json!({"event": "availability_found", "date": avail.date.to_string(), "booking_url": avail.booking_url}));
                            
                            if self.handle_found_date(avail).await? {
                                return Ok(()); 
                            }
                        }
                    }
                }
            }

            check_count += 1;

            if self.config.check_window > Duration::from_secs(60) && check_count % self.config.rest_cycle_checks == 0 {
                sleep(self.config.rest_cycle_duration).await;
            }

            sleep(self.config.check_interval).await;
        }

        info!("{}", json!({"event": "check_window_expired"}));
        Ok(())
    }

    async fn handle_found_date(&self, availability: crate::scraper::Availability) -> Result<bool> {
        let is_preferred = self.config.preferred_days.contains(&availability.date.format("%A").to_string());
        
        match self.config.mode {
            Mode::Alert => {
                self.send_ntfy(&availability).await?;
                Ok(true) 
            }
            Mode::Booking => {
                if is_preferred {
                    let client = self.client_pool.next();
                    let site = self.config.get_active_site().unwrap();
                    let creds = self.config.get_selected_credential().ok_or_else(|| anyhow!("No credential selected"))?;
                    
                    match Booker::attempt_booking(&client, site, creds, &availability.booking_url).await {
                        Ok(_) => Ok(true),
                        Err(_) => Ok(false),
                    }
                } else {
                    Ok(false) 
                }
            }
        }
    }

    async fn send_ntfy(&self, availability: &crate::scraper::Availability) -> Result<()> {
        let client = self.client_pool.get_default();
        let topic = &self.config.ntfy_topic;
        let url = format!("https://ntfy.sh/{}", topic);
        
        let title = format!("Pass Available: {}", availability.date);
        let message = format!("Found a pass for {}! Booking link: {}", availability.date, availability.booking_url);

        let _ = client.post(&url)
            .header("Title", title.clone())
            .header("Tags", "ticket,library")
            .body(message)
            .send()
            .await;

        info!("{}", json!({"event": "notification_sent", "title": title, "topic": topic}));
        Ok(())
    }
}
