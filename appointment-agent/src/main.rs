/*
 * Version: 0.1.1
 * Description: Main entry point for the Appointment Agent.
 * Ties together CLI parsing, logging, web dashboard, and the orchestration engine.
 */

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod config;
mod client_pool;
mod scraper;
mod booker;
mod agent;
mod web;

use crate::config::Config;
use crate::client_pool::ClientPool;
use crate::agent::Agent;

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
    // 1. Initialize Logging (Requirement 2.7)
    // We use JSON formatting as required by the Logging Contract.
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .json() // Mandatory JSON output for automated log parsing
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

    // 4. Initialize Client Pool (Requirement 2.8)
    let client_pool = ClientPool::new()
        .context("Failed to initialize stealth client pool")?;

    // 5. Execution Strategy
    if args.web {
        // Run with Web Dashboard (Requirement 2.4)
        info!("Starting agent in web mode...");
        
        let web_config = config.clone();
        let web_path = config_path.clone();
        
        // Spawn web server in the background
        let web_handle = tokio::spawn(async move {
            if let Err(e) = web::WebServer::run(web_config, web_path).await {
                eprintln!("Web server error: {}", e);
            }
        });

        // Initialize Agent
        let agent = Agent::new(config, client_pool);
        
        // In web mode, we run the agent. If it finishes, the web server stays up.
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
        // Run in CLI mode
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
