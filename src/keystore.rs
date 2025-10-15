use anyhow::{Context, Result};
use keyring::Entry;
use tracing::{debug, error, warn};

const SERVICE_NAME: &str = "mongo-copy";

pub struct KeyStore;

impl KeyStore {
    /// Store a URI securely in the system keyring
    pub fn store_uri(name: &str, uri: &str) -> Result<()> {
        debug!("Storing URI in keyring for: {}", name);

        let entry = Entry::new(SERVICE_NAME, name).context("Failed to create keyring entry")?;

        entry
            .set_password(uri)
            .context("Failed to store URI in keyring")?;

        debug!("URI stored successfully in keyring: {}", name);
        Ok(())
    }

    /// Retrieve a URI from the system keyring
    pub fn get_uri(name: &str) -> Result<Option<String>> {
        debug!("Retrieving URI from keyring for: {}", name);

        let entry = Entry::new(SERVICE_NAME, name).context("Failed to create keyring entry")?;

        match entry.get_password() {
            Ok(uri) => {
                debug!("URI retrieved successfully from keyring: {}", name);
                Ok(Some(uri))
            }
            Err(keyring::Error::NoEntry) => {
                debug!("No URI found in keyring for: {}", name);
                Ok(None)
            }
            Err(e) => {
                warn!("Failed to retrieve URI from keyring for {}: {}", name, e);
                Err(e).context("Failed to retrieve URI from keyring")
            }
        }
    }

    /// Delete a URI from the system keyring
    pub fn delete_uri(name: &str) -> Result<bool> {
        debug!("Deleting URI from keyring for: {}", name);

        let entry = Entry::new(SERVICE_NAME, name).context("Failed to create keyring entry")?;

        match entry.delete_credential() {
            Ok(_) => {
                debug!("URI deleted successfully from keyring: {}", name);
                Ok(true)
            }
            Err(keyring::Error::NoEntry) => {
                debug!("No URI found in keyring to delete: {}", name);
                Ok(false)
            }
            Err(e) => {
                error!("Failed to delete URI from keyring for {}: {}", name, e);
                Err(e).context("Failed to delete URI from keyring")
            }
        }
    }

    /// Check if a URI exists in the keyring
    #[allow(dead_code)]
    pub fn has_uri(name: &str) -> bool {
        let entry = match Entry::new(SERVICE_NAME, name) {
            Ok(e) => e,
            Err(_) => return false,
        };

        entry.get_password().is_ok()
    }
}
