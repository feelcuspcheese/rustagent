mod agent;
mod booker;
mod client_pool;
mod config;
mod scraper;
mod web;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::agent::Agent;
use crate::config::Config;

#[derive(Parser, Debug)]
#[command(name = "appointment-agent")]
#[command(about = "Library appointment booking agent")]
struct Args {
    /// Path to config file
    #[arg(short, long, default_value = "configs/config.yaml")]
    config: PathBuf,

    /// Run in web mode with dashboard
    #[arg(long)]
    web: bool,

    /// Run in dry-run mode (no actual bookings)
    #[arg(long)]
    dry_run: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    let args = Args::parse();

    // Load configuration
    let config = match Config::load(&args.config) {
        Ok(c) => {
            tracing::info!("Configuration loaded from {:?}", args.config);
            c
        }
        Err(e) => {
            tracing::warn!("Failed to load config from {:?}: {}", args.config, e);
            tracing::info!("Using default configuration");
            create_default_config()?
        }
    };

    // Create broadcast channel for logs
    let (log_tx, _log_rx) = broadcast::channel::<agent::AgentEvent>(1000);

    if args.web {
        // Run with web server
        run_with_web(config, log_tx).await?;
    } else {
        // Run agent directly
        run_agent(config, log_tx).await?;
    }

    Ok(())
}

async fn run_agent(config: Config, log_tx: broadcast::Sender<agent::AgentEvent>) -> Result<()> {
    let agent = Agent::new(config, log_tx)?;
    agent.run().await?;
    Ok(())
}

async fn run_with_web(
    config: Config,
    log_tx: broadcast::Sender<agent::AgentEvent>,
) -> Result<()> {
    // Clone config for web server
    let config_clone = config.clone();
    let log_tx_clone = log_tx.clone();

    // Spawn web server
    let web_handle = tokio::spawn(async move {
        if let Err(e) = web::run_webserver(Some(config_clone), log_tx_clone).await {
            tracing::error!("Web server error: {}", e);
        }
    });

    // Run agent
    let agent = Agent::new(config, log_tx)?;
    tokio::select! {
        result = agent.run() => {
            if let Err(e) = result {
                tracing::error!("Agent error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Shutting down...");
        }
    }

    web_handle.abort();
    Ok(())
}

fn create_default_config() -> Result<Config> {
    use std::collections::HashMap;
    use crate::config::{Mode, Site, LoginForm, BookingForm, Museum, Credential};

    let mut museums = HashMap::new();
    museums.insert(
        "sam".to_string(),
        Museum {
            name: "Seattle Art Museum".to_string(),
            slug: "SAM".to_string(),
            museumid: "7f2ac5c414b2".to_string(),
        },
    );

    let mut site_museums = HashMap::new();
    site_museums.insert(
        "sam".to_string(),
        Museum {
            name: "Seattle Art Museum".to_string(),
            slug: "SAM".to_string(),
            museumid: "7f2ac5c414b2".to_string(),
        },
    );

    let mut sites = HashMap::new();
    sites.insert(
        "spl".to_string(),
        Site {
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
                authidselector: "input[name='auth_id']".to_string(),
                loginurlselector: "input[name='login_url']".to_string(),
            },
            bookingform: BookingForm {
                actionurl: String::new(),
                emailfield: "email".to_string(),
            },
            museums: site_museums,
            preferredslug: "sam".to_string(),
        },
    );

    let mut credentials = HashMap::new();
    credentials.insert(
        "my_card".to_string(),
        Credential {
            name: "My Library Card".to_string(),
            username: "123456".to_string(),
            password: "PIN".to_string(),
            email: "me@example.com".to_string(),
            site: "spl".to_string(),
        },
    );

    Ok(Config {
        active_site: "spl".to_string(),
        mode: Mode::Alert,
        preferred_days: vec!["Saturday".to_string(), "Sunday".to_string()],
        strike_time: "09:00".to_string(),
        check_window: humantime::Duration::from_secs(60),
        check_interval: humantime::Duration::from_secs(2),
        pre_warm_offset: humantime::Duration::from_secs(30),
        request_jitter: humantime::Duration::from_secs(2),
        months_to_check: 2,
        rest_cycle_checks: 20,
        rest_cycle_duration: humantime::Duration::from_secs(3),
        ntfy_topic: "myappointments".to_string(),
        credentials,
        selected_credential: "my_card".to_string(),
        sites,
    })
}
