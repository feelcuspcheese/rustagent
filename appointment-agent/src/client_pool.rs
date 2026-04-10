/*
 * Version: 0.1.1
 * Description: HTTP client pool with browser impersonation.
 * Strictly adheres to REQUIREMENT.md v2.0 Section 2.8 (Stealth Requirements).
 */

use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use wreq::{Browser, Client};

/// ClientPool manages a collection of HTTP clients that impersonate real browsers.
/// This is essential for bypassing basic bot detection and maintaining TLS fingerprints.
pub struct ClientPool {
    clients: Vec<Arc<Client>>,
}

impl ClientPool {
    /// Creates a new ClientPool with a single Chrome 124 impersonated client.
    /// 
    /// Requirement 2.8: Use wreq with Browser::Chrome124.
    /// Requirement 2.5: Cookies must be saved to follow redirects after login.
    pub fn new() -> Result<Self> {
        // Build the primary client profile
        let client = Client::builder()
            .impersonate(Browser::Chrome124)
            .cookie_store(true) // Crucial for maintaining login session
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build wreq client: {}", e))?;

        Ok(Self {
            clients: vec![Arc::new(client)],
        })
    }

    /// Returns a reference to the next available client.
    /// Currently implements a simple reference to the primary client, 
    /// as per Requirement 2.8 (single profile per run).
    pub fn next(&self) -> Arc<Client> {
        // Simple implementation: always return the first client.
        // Can be extended to round-robin if multiple profiles are added.
        Arc::clone(&self.clients[0])
    }

    /// Helper to get a direct reference to the underlying client
    pub fn get_default(&self) -> &Client {
        &self.clients[0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_initialization() {
        let pool = ClientPool::new();
        assert!(pool.is_ok(), "ClientPool should initialize successfully with boring-sys backend");
    }
}
