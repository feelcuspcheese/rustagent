use anyhow::{Result, anyhow};
use tracing::{debug, error, info, warn};
use wreq::Client;

use crate::config::Credential;
use crate::scraper::{BookingFormFields, LoginFormFields, Scraper};

pub struct Booker {
    client: Client,
}

impl Booker {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Perform login using extracted form fields and credentials
    pub async fn login(
        &self,
        login_fields: &LoginFormFields,
        credential: &Credential,
    ) -> Result<bool> {
        info!("Attempting login");

        let mut retries = 0;
        let max_retries = 2;

        loop {
            let response = self
                .client
                .post(&login_fields.action_url)
                .form(&[
                    ("auth_id", &login_fields.auth_id),
                    ("login_url", &login_fields.login_url),
                    ("username", &credential.username),
                    ("password", &credential.password),
                ])
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() || status.is_redirection() {
                        info!("Login successful");
                        return Ok(true);
                    } else if status.is_client_error() || status.is_server_error() {
                        warn!("Login failed with status: {}", status);
                        retries += 1;
                        if retries >= max_retries {
                            error!("Login failed after {} retries", max_retries);
                            return Ok(false);
                        }
                        // Retry with same client (cookies persist)
                        continue;
                    } else {
                        info!("Login returned unexpected status: {}", status);
                        return Ok(true); // Assume success for non-error statuses
                    }
                }
                Err(e) => {
                    error!("Login request failed: {}", e);
                    retries += 1;
                    if retries >= max_retries {
                        return Err(anyhow!("Login failed after retries: {}", e));
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            }
        }
    }

    /// Attempt to book an appointment
    pub async fn book(
        &self,
        booking_fields: &BookingFormFields,
        credential: &Credential,
        success_indicator: &str,
    ) -> Result<bool> {
        info!("Attempting booking");

        // Build form data from hidden fields
        let mut form_data: Vec<(&str, &str)> = booking_fields
            .hidden_fields
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        // Add email field if present
        if let Some(email_field) = &booking_fields.email_field_name {
            form_data.push((email_field.as_str(), &credential.email));
        }

        // Resolve action URL if relative
        let action_url = if booking_fields.action_url.starts_with("http") {
            booking_fields.action_url.clone()
        } else {
            // This would need base URL from context - for now assume absolute or handle in caller
            booking_fields.action_url.clone()
        };

        let response = self.client.post(&action_url).form(&form_data).send().await?;
        let html = response.text().await?;

        // Check for success indicator
        let scraper = Scraper::new(self.client.clone());
        if scraper.check_success(&html, success_indicator) {
            info!("Booking successful!");
            Ok(true)
        } else {
            debug!("Booking response snippet: {}", &html[..html.len().min(500)]);
            Err(anyhow!("Booking failed - success indicator not found"))
        }
    }

    /// Follow a booking URL and determine if login is required
    pub async fn follow_booking_url(&self, url: &str) -> Result<BookingPageState> {
        info!("Following booking URL: {}", url);
        
        let response = self.client.get(url).send().await?;
        let html = response.text().await?;

        let scraper = Scraper::new(self.client.clone());
        
        // Check if login form is present
        if let Some(login_fields) = scraper.detect_login_form(&html) {
            info!("Login required");
            Ok(BookingPageState::LoginRequired(login_fields))
        } else if let Some(booking_fields) = scraper.detect_booking_form(&html) {
            info!("Booking form found");
            Ok(BookingPageState::BookingReady(booking_fields))
        } else {
            warn!("Could not detect form type on booking page");
            Ok(BookingPageState::Unknown(html))
        }
    }
}

#[derive(Debug)]
pub enum BookingPageState {
    LoginRequired(LoginFormFields),
    BookingReady(BookingFormFields),
    Unknown(String),
}
