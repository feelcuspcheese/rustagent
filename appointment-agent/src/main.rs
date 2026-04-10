/*
 * Version: 0.1.5
 * Description: Main entry point. Updated to use the modules via the library crate.
 */

use anyhow::{Context, Result};
use clap::Parser;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

// Use the library crate members
use appointment_agent::config::Config;
use appointment_agent::client_pool::ClientPool;
use appointment_agent::agent::Agent;
use appointment_agent::web;

#[derive(Parser, Debug)]
#[command(author, version, about = "Rust Appointment Agent for LibCal systems")]
struct Args {
    /// Path to the configuration file
    #[arg(short, long, default_value = "configs/config.yaml")]
    config: String,

    /// Enable the web dashboard
    #[arg(short, long, default_value_t = false)]
    web: bool,

    /// Dry-run mode (skips actual booking/notifications)
    #[arg(short, long, default_value_t = false)]
    dry_run: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialize Logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .json() 
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set tracing subscriber")?;

    // 2. Parse CLI Arguments
    let args = Args::parse();

    // 3. Load Configuration
    let config_path = args.config.clone();
    let config = Config::load(&config_path)
        .with_context(|| format!("Failed to load config from {}", config_path))?;
    
    info!("Configuration loaded from {}", config_path);

    // 4. Initialize Client Pool
    let client_pool = ClientPool::new()
        .context("Failed to initialize stealth client pool")?;

    // 5. Execution Strategy
    if args.web {
        info!("Starting agent in web mode...");
        
        let web_config = config.clone();
        let web_path = config_path.clone();
        
        let web_handle = tokio::spawn(async move {
            if let Err(e) = web::WebServer::run(web_config, web_path).await {
                eprintln!("Web server error: {}", e);
            }
        });

        let agent = Agent::new(config, client_pool);
        
        tokio::select! {
            agent_res = agent.run() => {
                if let Err(e) = agent_res {
                    tracing::error!("Agent execution failed: {}", e);
                }
            }
            _ = web_handle => {
                info!("Web server shut down.");
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Shutdown signal received.");
            }
        }
    } else {
        info!("Starting agent in CLI mode...");
        let agent = Agent::new(config, client_pool);
        
        tokio::select! {
            res = agent.run() => res?,
            _ = tokio::signal::ctrl_c() => {
                info!("Shutdown signal received.");
            }
        }
    }

    Ok(())
}
