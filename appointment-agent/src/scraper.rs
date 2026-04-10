/*
 * Version: 0.1.1
 * Description: Logic for constructing availability URLs and parsing LibCal HTML responses.
 * Strictly adheres to REQUIREMENT.md v2.0 Section 2.2 (URL Construction) and 2.3 (Parsing Rules).
 */

use anyhow::{Context, Result};
use chrono::NaiveDate;
use scraper::{Html, Selector};
use url::Url;
use crate::config::SiteConfig;

#[derive(Debug, Clone)]
pub struct Availability {
    pub date: NaiveDate,
    pub booking_url: String,
    pub day_text: String,
}

pub struct Scraper;

impl Scraper {
    /// Constructs the availability URL based on Requirement 2.2.
    /// Format: {baseurl}{availabilityendpoint}?museum={museumid}&date={YYYY-MM-DD}&digital={digital}&physical={physical}&location={location}
    pub fn build_availability_url(
        site: &SiteConfig,
        museum_id: &str,
        date: NaiveDate,
    ) -> Result<String> {
        let base = Url::parse(&site.baseurl)
            .with_context(|| format!("Invalid base URL: {}", site.baseurl))?;
        
        let mut url = base.join(&site.availabilityendpoint)
            .with_context(|| format!("Invalid endpoint: {}", site.availabilityendpoint))?;

        url.query_pairs_mut()
            .append_pair("museum", museum_id)
            .append_pair("date", &date.format("%Y-%m-%d").to_string())
            .append_pair("digital", &site.digital.to_string())
            .append_pair("physical", &site.physical.to_string())
            .append_pair("location", &site.location);

        Ok(url.to_string())
    }

    /// Parses the HTML response to find available booking slots.
    /// Requirement 2.3: Use CSS selector bookinglinkselector.
    pub fn parse_availability(
        html_content: &str,
        site: &SiteConfig,
    ) -> Result<Vec<Availability>> {
        let document = Html::parse_document(html_content);
        let selector = Selector::parse(&site.bookinglinkselector)
            .map_err(|_| anyhow::anyhow!("Invalid CSS selector: {}", site.bookinglinkselector))?;

        let mut results = Vec::new();
        let base_url = Url::parse(&site.baseurl)?;

        for element in document.select(&selector) {
            // Requirement 2.3: Extract href -> booking_url
            let href = match element.value().attr("href") {
                Some(h) => h,
                None => continue,
            };

            // Requirement 2.2: Resolve relative booking URLs
            let booking_url = base_url.join(href)?.to_string();

            // Requirement 2.3: Extract inner text -> date (day number)
            let inner_text = element.text().collect::<String>().trim().to_string();

            // Requirement 2.4.2: Extract full date from the booking URL query parameter 'date'
            let parsed_url = Url::parse(&booking_url)?;
            let date_str = parsed_url.query_pairs()
                .find(|(key, _)| key == "date")
                .map(|(_, value)| value.to_string());

            if let Some(ds) = date_str {
                if let Ok(parsed_date) = NaiveDate::parse_from_str(&ds, "%Y-%m-%d") {
                    results.push(Availability {
                        date: parsed_date,
                        booking_url,
                        day_text: inner_text,
                    });
                }
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{SiteConfig, LoginFormConfig, BookingFormConfig};
    use std::collections::HashMap;

    fn mock_site() -> SiteConfig {
        SiteConfig {
            name: "Test Library".into(),
            baseurl: "https://test.libcal.com".into(),
            availabilityendpoint: "/pass/availability".into(),
            digital: true,
            physical: false,
            location: "0".into(),
            bookinglinkselector: "a.s-lc-pass-available".into(),
            successindicator: "Thank you!".into(),
            loginform: LoginFormConfig {
                usernamefield: "u".into(),
                passwordfield: "p".into(),
                submitbutton: "s".into(),
                csrfselector: "".into(),
                authidselector: "input[name='auth_id']".into(),
                loginurlselector: "input[name='login_url']".into(),
            },
            bookingform: BookingFormConfig {
                actionurl: "".into(),
                emailfield: "email".into(),
            },
            museums: HashMap::new(),
            preferredslug: "sam".into(),
        }
    }

    #[test]
    fn test_url_construction() {
        let site = mock_site();
        let date = NaiveDate::from_ymd_opt(2026, 4, 15).unwrap();
        let url = Scraper::build_availability_url(&site, "7f2ac5c414b2", date).unwrap();
        
        assert!(url.contains("date=2026-04-15"));
        assert!(url.contains("museum=7f2ac5c414b2"));
        assert!(url.contains("digital=true"));
    }

    #[test]
    fn test_parsing() {
        let site = mock_site();
        let html = r#"
            <div class="day day-2026-04-15">
                <a href="/passes/SAM/book?date=2026-04-15&pass=abc" class="s-lc-pass-available">15</a>
            </div>
        "#;
        
        let results = Scraper::parse_availability(html, &site).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].date.to_string(), "2026-04-15");
        assert_eq!(results[0].booking_url, "https://test.libcal.com/passes/SAM/book?date=2026-04-15&pass=abc");
    }
}
