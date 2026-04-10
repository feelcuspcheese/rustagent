use appointment_agent::config::{Config, Mode};
use appointment_agent::scraper::Scraper;
use appointment_agent::client_pool::ClientPool;

#[test]
fn test_config_parsing() {
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
    assert_eq!(config.preferred_days, vec!["Saturday", "Sunday"]);
}

#[test]
fn test_url_construction() {
    use chrono::NaiveDate;
    use std::collections::HashMap;
    use appointment_agent::config::{Site, Museum, LoginForm, BookingForm};

    let mut museums = HashMap::new();
    museums.insert(
        "sam".to_string(),
        Museum {
            name: "Seattle Art Museum".to_string(),
            slug: "SAM".to_string(),
            museumid: "7f2ac5c414b2".to_string(),
        },
    );

    let site = Site {
        name: "Seattle Public Library".to_string(),
        baseurl: "https://spl.libcal.com".to_string(),
        availabilityendpoint: "/pass/availability/institution".to_string(),
        digital: true,
        physical: false,
        location: "0".to_string(),
        bookinglinkselector: "a.s-lc-pass-availability".to_string(),
        successindicator: "Thank you!".to_string(),
        loginform: LoginForm {
            usernamefield: "username".to_string(),
            passwordfield: "password".to_string(),
            submitbutton: "submit".to_string(),
            csrfselector: String::new(),
            authidselector: String::new(),
            loginurlselector: String::new(),
        },
        bookingform: BookingForm {
            actionurl: String::new(),
            emailfield: "email".to_string(),
        },
        museums,
        preferredslug: "sam".to_string(),
    };

    let museum = site.museums.get("sam").unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 4, 15).unwrap();

    let url = Scraper::construct_availability_url(&site, museum, date);
    
    assert!(url.contains("museum=7f2ac5c414b2"));
    assert!(url.contains("date=2026-04-15"));
    assert!(url.contains("digital=true"));
    assert!(url.contains("physical=false"));
    assert!(url.contains("location=0"));
}

#[tokio::test]
async fn test_client_pool_creation() {
    let pool = ClientPool::new().unwrap();
    assert_eq!(pool.clients.len(), 1);
}

#[test]
fn test_availability_parsing() {
    use chrono::NaiveDate;
    use std::collections::HashMap;
    use appointment_agent::config::{Site, Museum, LoginForm, BookingForm};

    let html = r#"
<div class="s-lc-pass-calendar by-museum">
  <div class="day day-Wed day-2026-04-15">
    <a href="/passes/SAM/book?date=2026-04-15&pass=abc123" class="s-lc-pass-availability s-lc-pass-digital s-lc-pass-available">
      15
    </a>
  </div>
</div>
"#;

    let mut museums = HashMap::new();
    museums.insert(
        "sam".to_string(),
        Museum {
            name: "Seattle Art Museum".to_string(),
            slug: "SAM".to_string(),
            museumid: "7f2ac5c414b2".to_string(),
        },
    );

    let site = Site {
        name: "Seattle Public Library".to_string(),
        baseurl: "https://spl.libcal.com".to_string(),
        availabilityendpoint: "/pass/availability/institution".to_string(),
        digital: true,
        physical: false,
        location: "0".to_string(),
        bookinglinkselector: "a.s-lc-pass-availability.s-lc-pass-digital.s-lc-pass-available".to_string(),
        successindicator: "Thank you!".to_string(),
        loginform: LoginForm {
            usernamefield: "username".to_string(),
            passwordfield: "password".to_string(),
            submitbutton: "submit".to_string(),
            csrfselector: String::new(),
            authidselector: String::new(),
            loginurlselector: String::new(),
        },
        bookingform: BookingForm {
            actionurl: String::new(),
            emailfield: "email".to_string(),
        },
        museums,
        preferredslug: "sam".to_string(),
    };

    // Create a dummy client for the scraper
    let client = ClientPool::new().unwrap().next().clone();
    let scraper = Scraper::new(client);
    let base_date = NaiveDate::from_ymd_opt(2026, 4, 1).unwrap();
    
    let slots = scraper.parse_availability(html, &site, base_date).unwrap();
    
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].date, "2026-04-15");
    assert!(slots[0].booking_url.contains("/passes/SAM/book"));
}

#[test]
fn test_login_form_detection() {
    let html = r#"
<form action="https://libauth.com/form_login" method="post">
  <input type="hidden" name="auth_id" value="3001">
  <input type="hidden" name="login_url" value="https://spl.libapps.com/libapps/libauth?auth_id=3001">
  <input type="text" name="username">
  <input type="password" name="password">
  <button type="submit">Login</button>
</form>
"#;

    let client = ClientPool::new().unwrap().next().clone();
    let scraper = Scraper::new(client);
    
    let form = scraper.detect_login_form(html).unwrap();
    
    assert_eq!(form.auth_id, "3001");
    assert_eq!(form.login_url, "https://spl.libapps.com/libapps/libauth?auth_id=3001");
    assert_eq!(form.action_url, "https://libauth.com/form_login");
}

#[test]
fn test_booking_form_detection() {
    let html = r#"
<form id="s-lc-bform" method="post" action="/passes/7f2ac5c414b2/book">
  <input type="hidden" name="museum" value="7f2ac5c414b2">
  <input type="hidden" name="pass" value="63ea409f9fab">
  <input type="hidden" name="date" value="2026-04-15">
  <input type="hidden" name="crc" value="d50ab18d9fa26567aaee1106311fa618">
  <div class="form-group">
    <input type="email" name="email" required>
  </div>
  <button type="submit">Submit my Booking</button>
</form>
"#;

    let client = ClientPool::new().unwrap().next().clone();
    let scraper = Scraper::new(client);
    
    let form = scraper.detect_booking_form(html).unwrap();
    
    assert_eq!(form.action_url, "/passes/7f2ac5c414b2/book");
    assert_eq!(form.hidden_fields.len(), 4);
    assert_eq!(form.email_field_name, Some("email".to_string()));
}

#[test]
fn test_success_indicator() {
    let html = r#"
<h1 id="s-lc-eq-success-title">Thank you!</h1>
<p>The following Digital Pass reservation was made:</p>
"#;

    let client = ClientPool::new().unwrap().next().clone();
    let scraper = Scraper::new(client);
    
    assert!(scraper.check_success(html, "Thank you!"));
    assert!(!scraper.check_success(html, "Not found"));
}
