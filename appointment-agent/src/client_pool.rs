use anyhow::Result;
use std::sync::Arc;
use wreq::{Client, Browser};

pub struct ClientPool {
    clients: Vec<Arc<Client>>,
}

impl ClientPool {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .impersonate(Browser::Chrome124)
            .cookie_store(true)
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self { 
            clients: vec![Arc::new(client)] 
        })
    }

    pub fn next(&self) -> &Client {
        &self.clients[0]
    }

    #[allow(dead_code)]
    pub fn get_client(&self, _index: usize) -> &Client {
        &self.clients[0]
    }
}

impl Default for ClientPool {
    fn default() -> Self {
        Self::new().expect("Failed to create client pool")
    }
}
