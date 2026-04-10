use anyhow::{Result, anyhow};
use chrono::{Datelike, Local, NaiveDate};
use scraper::{Html, Selector};
use tracing::{debug, error, info, warn};
use wreq::Client;

use crate::config::{Config, Museum, Site};

#[derive(Debug, Clone)]
pub struct AvailabilitySlot {
    pub date: String,
    pub booking_url: String,
}

pub struct Scraper {
    client: Client,
}

impl Scraper {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Construct availability URL according to the specification
    pub fn construct_availability_url(
        site: &Site,
        museum: &Museum,
        date: NaiveDate,
    ) -> String {
        format!(
            "{}{}?museum={}&date={}&digital={}&physical={}&location={}",
            site.baseurl,
            site.availabilityendpoint,
            museum.museumid,
            date.format("%Y-%m-%d"),
            site.digital,
            site.physical,
            site.location
        )
    }

    /// Fetch and parse availability for a given month
    pub async fn fetch_availability(
        &self,
        site: &Site,
        museum: &Museum,
        date: NaiveDate,
    ) -> Result<Vec<AvailabilitySlot>> {
        let url = Self::construct_availability_url(site, museum, date);
        
        info!(url = %url, month = %date.format("%Y-%m"), "Fetching availability");

        let response = self.client.get(&url).send().await?;
        let html = response.text().await?;
        
        self.parse_availability(&html, site, date)
    }

    /// Parse HTML to extract available slots
    pub fn parse_availability(
        &self,
        html: &str,
        site: &Site,
        base_date: NaiveDate,
    ) -> Result<Vec<AvailabilitySlot>> {
        let document = Html::parse_document(html);
        
        let selector = Selector::parse(&site.bookinglinkselector)
            .map_err(|e| anyhow!("Invalid selector '{}': {}", site.bookinglinkselector, e))?;

        let mut slots = Vec::new();

        for element in document.select(&selector) {
            if let Some(href) = element.value().attr("href") {
                let day_text = element.text().collect::<String>().trim().to_string();
                
                // Extract full date from href query parameter or construct from day number
                let full_date = if let Some(parsed) = url::Url::parse(href).ok().and_then(|u| {
                    u.query_pairs().find(|(k, _)| k == "date").map(|(_, v)| v.to_string())
                }) {
                    parsed
                } else {
                    // Construct date from day number and base month/year
                    if let Ok(day) = day_text.parse::<u32>() {
                        NaiveDate::from_ymd_opt(base_date.year(), base_date.month(), day)
                            .map(|d| d.format("%Y-%m-%d").to_string())
                            .unwrap_or_else(|| format!("{}-{:02}-{}", base_date.year(), base_date.month(), day))
                    } else {
                        continue;
                    }
                };

                // Resolve relative URLs
                let booking_url = if href.starts_with("http") {
                    href.to_string()
                } else {
                    format!("{}{}", site.baseurl, href)
                };

                info!(date = %full_date, booking_url = %booking_url, "Availability found");
                slots.push(AvailabilitySlot {
                    date: full_date,
                    booking_url,
                });
            }
        }

        if slots.is_empty() {
            debug!("No availability found for {}", base_date.format("%Y-%m"));
        }

        Ok(slots)
    }

    /// Detect login form in HTML response
    pub fn detect_login_form(&self, html: &str) -> Option<LoginFormFields> {
        let document = Html::parse_document(html);
        
        // Look for form with action containing "form_login"
        let form_selector = Selector::parse("form[action*='form_login']").ok()?;
        let form = document.select(&form_selector).next()?;

        let mut auth_id = None;
        let mut login_url = None;
        let mut action_url = None;

        // Extract form action
        if let Some(action) = form.value().attr("action") {
            action_url = Some(action.to_string());
        }

        // Extract hidden inputs
        let input_selector = Selector::parse("input[type='hidden']").ok()?;
        for input in form.select(&input_selector) {
            let name = input.value().attr("name")?;
            let value = input.value().attr("value")?;
            
            match name {
                "auth_id" => auth_id = Some(value.to_string()),
                "login_url" => login_url = Some(value.to_string()),
                _ => {}
            }
        }

        if auth_id.is_some() && login_url.is_some() {
            info!("Login form detected");
            Some(LoginFormFields {
                auth_id: auth_id?,
                login_url: login_url?,
                action_url: action_url?,
            })
        } else {
            None
        }
    }

    /// Detect booking form in HTML response
    pub fn detect_booking_form(&self, html: &str) -> Option<BookingFormFields> {
        let document = Html::parse_document(html);
        
        // Look for form#s-lc-bform
        let form_selector = Selector::parse("form#s-lc-bform").ok()?;
        let form = document.select(&form_selector).next()?;

        let mut hidden_fields = Vec::new();
        let mut action_url = None;
        let mut email_field_name = None;

        // Extract form action
        if let Some(action) = form.value().attr("action") {
            action_url = Some(action.to_string());
        }

        // Extract all inputs
        let input_selector = Selector::parse("input").ok()?;
        for input in form.select(&input_selector) {
            let input_type = input.value().attr("type").unwrap_or("text");
            let name = input.value().attr("name")?;
            
            if input_type == "hidden" {
                if let Some(value) = input.value().attr("value") {
                    hidden_fields.push((name.to_string(), value.to_string()));
                }
            } else if input_type == "email" {
                email_field_name = Some(name.to_string());
            }
        }

        Some(BookingFormFields {
            action_url: action_url?,
            hidden_fields,
            email_field_name,
        })
    }

    /// Check if response contains success indicator
    pub fn check_success(&self, html: &str, indicator: &str) -> bool {
        html.contains(indicator)
    }

    /// Relaxed parsing when primary selector fails
    pub fn parse_availability_relaxed(&self, html: &str, site: &Site) -> Result<Vec<AvailabilitySlot>> {
        warn!("Using relaxed selector for availability parsing");
        let document = Html::parse_document(html);
        
        // Try generic 'a' selector as fallback
        let selector = Selector::parse("a").map_err(|e| anyhow!("Invalid relaxed selector: {}", e))?;
        
        let mut slots = Vec::new();
        for element in document.select(&selector) {
            if let Some(href) = element.value().attr("href") {
                if href.contains("/passes/") && href.contains("/book") {
                    let day_text = element.text().collect::<String>();
                    slots.push(AvailabilitySlot {
                        date: day_text.trim().to_string(),
                        booking_url: if href.starts_with("http") {
                            href.to_string()
                        } else {
                            format!("{}{}", site.baseurl, href)
                        },
                    });
                }
            }
        }

        Ok(slots)
    }
}

#[derive(Debug, Clone)]
pub struct LoginFormFields {
    pub auth_id: String,
    pub login_url: String,
    pub action_url: String,
}

#[derive(Debug, Clone)]
pub struct BookingFormFields {
    pub action_url: String,
    pub hidden_fields: Vec<(String, String)>,
    pub email_field_name: Option<String>,
}
