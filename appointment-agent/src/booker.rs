/*
 * Version: 0.1.5
 * Description: Login and booking flow. Fixed mut warning and type inference issues.
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
    pub async fn attempt_booking(
        client: &Client,
        site: &SiteConfig,
        creds: &CredentialConfig,
        booking_url: &str,
    ) -> Result<()> {
        info!("{}", json!({"event": "booking_attempt", "date": booking_url}));

        // Type annotation helps with type inference errors
        let resp: wreq::Response = client.get(booking_url).send().await?;
        let mut body = resp.text().await?;
        let current_url = booking_url.to_string(); // Removed mut as it's not changed

        if body.contains("form_login") {
            info!("{}", json!({"event": "login_required"}));
            body = Self::handle_login(client, site, creds, &body, &current_url).await?;
            info!("{}", json!({"event": "login_success"}));
        }

        let booking_result_body = Self::handle_booking_form(client, site, creds, &body, &current_url).await?;

        if booking_result_body.contains(&site.successindicator) {
            info!("{}", json!({"event": "booking_success", "date": booking_url}));
            Ok(())
        } else {
            let err_msg = "Success indicator not found in response";
            info!("{}", json!({"event": "booking_failed", "date": booking_url, "error": err_msg}));
            Err(anyhow!(err_msg))
        }
    }

    async fn handle_login(
        client: &Client,
        site: &SiteConfig,
        creds: &CredentialConfig,
        html_content: &str,
        page_url: &str,
    ) -> Result<String> {
        let document = Html::parse_document(html_content);
        
        let form_selector = Selector::parse("form[action*='form_login']").unwrap();
        let form_element = document.select(&form_selector).next()
            .ok_or_else(|| anyhow!("Login form not found"))?;

        let action = form_element.value().attr("action")
            .ok_or_else(|| anyhow!("Login form missing action attribute"))?;
        
        let base_url = Url::parse(page_url)?;
        let login_action_url = base_url.join(action)?.to_string();

        let auth_id = Self::extract_input_value(&document, &site.loginform.authidselector)?;
        let login_url_val = Self::extract_input_value(&document, &site.loginform.loginurlselector)?;

        let mut params = HashMap::new();
        params.insert(site.loginform.usernamefield.clone(), creds.username.clone());
        params.insert(site.loginform.passwordfield.clone(), creds.password.clone());
        params.insert("auth_id".to_string(), auth_id);
        params.insert("login_url".to_string(), login_url_val);
        params.insert(site.loginform.submitbutton.clone(), "1".to_string());

        let resp: wreq::Response = client.post(&login_action_url)
            .form(&params)
            .send()
            .await
            .context("Failed to submit login form")?;

        Ok(resp.text().await?)
    }

    async fn handle_booking_form(
        client: &Client,
        site: &SiteConfig,
        creds: &CredentialConfig,
        html_content: &str,
        page_url: &str,
    ) -> Result<String> {
        let document = Html::parse_document(html_content);
        
        let form_selector = Selector::parse("form#s-lc-bform").unwrap();
        let form_element = document.select(&form_selector).next()
            .ok_or_else(|| anyhow!("Booking form (s-lc-bform) not found"))?;

        let action = form_element.value().attr("action")
            .ok_or_else(|| anyhow!("Booking form missing action attribute"))?;
        
        let base_url = Url::parse(page_url)?;
        let booking_action_url = base_url.join(action)?.to_string();

        let mut params = HashMap::new();
        let hidden_selector = Selector::parse("input[type='hidden']").unwrap();
        for input in form_element.select(&hidden_selector) {
            if let (Some(name), Some(value)) = (input.value().attr("name"), input.value().attr("value")) {
                params.insert(name.to_string(), value.to_string());
            }
        }

        params.insert(site.bookingform.emailfield.clone(), creds.email.clone());

        let resp: wreq::Response = client.post(&booking_action_url)
            .form(&params)
            .send()
            .await
            .context("Failed to submit booking form")?;

        Ok(resp.text().await?)
    }

    fn extract_input_value(doc: &Html, selector_str: &str) -> Result<String> {
        let selector = Selector::parse(selector_str)
            .map_err(|_| anyhow!("Invalid selector: {}", selector_str))?;
        
        doc.select(&selector).next()
            .and_then(|el| el.value().attr("value"))
            .map(|v| v.to_string())
            .ok_or_else(|| anyhow!("Could not find value for selector: {}", selector_str))
    }
}
