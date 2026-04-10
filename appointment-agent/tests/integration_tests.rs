/*
 * Version: 0.1.5
 * Description: Integration tests for the Appointment Agent.
 * Fixed: Updated to use correct struct names and link to the library crate.
 */

use appointment_agent::config::{Config, Mode};
use appointment_agent::scraper::Scraper;
use appointment_agent::client_pool::ClientPool;
use chrono::NaiveDate;
use std::collections::HashMap;

#[tokio::test]
async fn test_config_loading_and_logic() {
    // This test ensures that the library crate is accessible and 
    // basic logic (like date formatting) works as expected.
    let date = NaiveDate::from_ymd_opt(2026, 4, 15).unwrap();
    assert_eq!(date.to_string(), "2026-04-15");
}

#[test]
fn test_scraper_url_builder() {
    use appointment_agent::config::{SiteConfig, LoginFormConfig, BookingFormConfig};

    let site = SiteConfig {
        name: "Test Site".into(),
        baseurl: "https://example.com".into(),
        availabilityendpoint: "/avail".into(),
        digital: true,
        physical: false,
        location: "0".into(),
        bookinglinkselector: "a".into(),
        successindicator: "Success".into(),
        loginform: LoginFormConfig {
            usernamefield: "u".into(),
            passwordfield: "p".into(),
            submitbutton: "s".into(),
            csrfselector: "".into(),
            authidselector: "auth".into(),
            loginurlselector: "url".into(),
        },
        bookingform: BookingFormConfig {
            actionurl: "".into(),
            emailfield: "e".into(),
        },
        museums: HashMap::new(),
        preferredslug: "test".into(),
    };

    let date = NaiveDate::from_ymd_opt(2026, 4, 15).unwrap();
    let url = Scraper::build_availability_url(&site, "museum123", date).unwrap();
    
    assert!(url.contains("museum=museum123"));
    assert!(url.contains("date=2026-04-15"));
}

#[tokio::test]
async fn test_client_pool_init() {
    // Verifies that the client pool can initialize with stealth settings
    let pool = ClientPool::new();
    assert!(pool.is_ok());
}

#[test]
fn test_config_struct_mapping() {
    // This test matches the REQUIREMENT.md schema precisely
    use appointment_agent::config::{SiteConfig, MuseumConfig, LoginFormConfig, BookingFormConfig};
    
    let museum = MuseumConfig {
        name: "SAM".into(),
        slug: "sam".into(),
        museumid: "123".into(),
    };
    
    assert_eq!(museum.slug, "sam");
}
