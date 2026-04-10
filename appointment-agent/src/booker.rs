/*
 * Version: 0.1.1
 * Description: Automated login and booking form submission logic.
 * Strictly adheres to REQUIREMENT.md v2.0 Section 2.3 (Parsing Rules) and 2.5 (Login & Booking Rules).
 */

use anyhow::{anyhow, Context, Result};
use scraper::{Html, Selector};
use serde_json::json;
use std::collections::HashMap;
use tracing::info;
use url::Url;
use wreq::Client;

use crate::config::{CredentialConfig, SiteConfig};

pub struct Booker;

impl Booker {
    /// Executes the full booking flow: Login (if needed) -> Form Submission -> Verification.
    pub async fn attempt_booking(
        client: &Client,
        site: &SiteConfig,
        creds: &CredentialConfig,
        booking_url: &str,
    ) -> Result<()> {
        info!("{}", json!({"event": "booking_attempt", "date": booking_url}));

        // 1. Navigate to booking page (handles redirects automatically)
        let resp = client.get(booking_url).send().await?;
        let mut body = resp.text().await?;
        let mut current_url = booking_url.to_string();

        // 2. Check for Login Form (Requirement 2.3 & 2.5)
        if body.contains("form_login") {
            info!("{}", json!({"event": "login_required"}));
            body = Self::handle_login(client, site, creds, &body, &current_url).await?;
            info!("{}", json!({"event": "login_success"}));
        }

        // 3. Process Booking Form (Requirement 2.3 & 2.5)
        let booking_result_body = Self::handle_booking_form(client, site, creds, &body, &current_url).await?;

        // 4. Verify Success (Requirement 2.5)
        if booking_result_body.contains(&site.successindicator) {
            info!("{}", json!({"event": "booking_success", "date": booking_url}));
            Ok(())
        } else {
            let err_msg = "Success indicator not found in response";
            info!("{}", json!({"event": "booking_failed", "date": booking_url, "error": err_msg}));
            Err(anyhow!(err_msg))
        }
    }

    /// Detects and submits the login form.
    async fn handle_login(
        client: &Client,
        site: &SiteConfig,
        creds: &CredentialConfig,
        html_content: &str,
        page_url: &str,
    ) -> Result<String> {
        let document = Html::parse_document(html_content);
        
        // Find form whose action contains "form_login"
        let form_selector = Selector::parse("form[action*='form_login']").unwrap();
        let form_element = document.select(&form_selector).next()
            .ok_or_else(|| anyhow!("Login form not found even though 'form_login' detected in body"))?;

        let action = form_element.value().attr("action")
            .ok_or_else(|| anyhow!("Login form missing action attribute"))?;
        
        let base_url = Url::parse(page_url)?;
        let login_action_url = base_url.join(action)?.to_string();

        // Extract hidden fields: auth_id and login_url (Requirement 2.1 / 2.3)
        let auth_id = Self::extract_input_value(&document, &site.loginform.authidselector)?;
        let login_url_val = Self::extract_input_value(&document, &site.loginform.loginurlselector)?;

        // Build POST data
        let mut params = HashMap::new();
        params.insert(site.loginform.usernamefield.clone(), creds.username.clone());
        params.insert(site.loginform.passwordfield.clone(), creds.password.clone());
        params.insert("auth_id".to_string(), auth_id);
        params.insert("login_url".to_string(), login_url_val);
        params.insert(site.loginform.submitbutton.clone(), "1".to_string());

        // Perform login POST
        let resp = client.post(&login_action_url)
            .form(&params)
            .send()
            .await
            .context("Failed to submit login form")?;

        Ok(resp.text().await?)
    }

    /// Detects and submits the final booking form (s-lc-bform).
    async fn handle_booking_form(
        client: &Client,
        site: &SiteConfig,
        creds: &CredentialConfig,
        html_content: &str,
        page_url: &str,
    ) -> Result<String> {
        let document = Html::parse_document(html_content);
        
        // Requirement 2.3: Look for <form id="s-lc-bform">
        let form_selector = Selector::parse("form#s-lc-bform").unwrap();
        let form_element = document.select(&form_selector).next()
            .ok_or_else(|| anyhow!("Booking form (s-lc-bform) not found on page"))?;

        let action = form_element.value().attr("action")
            .ok_or_else(|| anyhow!("Booking form missing action attribute"))?;
        
        let base_url = Url::parse(page_url)?;
        let booking_action_url = base_url.join(action)?.to_string();

        // Requirement 2.3: Extract all hidden inputs
        let mut params = HashMap::new();
        let hidden_selector = Selector::parse("input[type='hidden']").unwrap();
        for input in form_element.select(&hidden_selector) {
            if let (Some(name), Some(value)) = (input.value().attr("name"), input.value().attr("value")) {
                params.insert(name.to_string(), value.to_string());
            }
        }

        // Requirement 2.3: Set the email field
        params.insert(site.bookingform.emailfield.clone(), creds.email.clone());

        // Perform booking POST
        let resp = client.post(&booking_action_url)
            .form(&params)
            .send()
            .await
            .context("Failed to submit booking form")?;

        Ok(resp.text().await?)
    }

    /// Utility to extract an input value by selector.
    fn extract_input_value(doc: &Html, selector_str: &str) -> Result<String> {
        let selector = Selector::parse(selector_str)
            .map_err(|_| anyhow!("Invalid selector: {}", selector_str))?;
        
        doc.select(&selector).next()
            .and_then(|el| el.value().attr("value"))
            .map(|v| v.to_string())
            .ok_or_else(|| anyhow!("Could not find value for selector: {}", selector_str))
    }
}
