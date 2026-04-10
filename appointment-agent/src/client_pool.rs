/*
 * Version: 0.1.5
 * Description: HTTP client pool with browser impersonation. Verified cookie_store availability.
 */

use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use wreq::{Client};
use wreq_util::Emulation;

/// ClientPool manages a collection of HTTP clients that impersonate real browsers.
pub struct ClientPool {
    clients: Vec<Arc<Client>>,
}

impl ClientPool {
    /// Creates a new ClientPool with a single Chrome 124 impersonated client.
    pub fn new() -> Result<Self> {
        // Build the primary client profile
        // Requirement 2.8: Use wreq with Emulation::Chrome124.
        let client = Client::builder()
            .emulation(Emulation::Chrome124)
            .cookie_store(true) // Enabled via 'cookies' feature in Cargo.toml
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build wreq client: {}", e))?;

        Ok(Self {
            clients: vec![Arc::new(client)],
        })
    }

    /// Returns a reference to the next available client.
    pub fn next(&self) -> Arc<Client> {
        Arc::clone(&self.clients[0])
    }

    /// Helper to get a direct reference to the underlying client
    pub fn get_default(&self) -> &Client {
        &self.clients[0]
    }
}
