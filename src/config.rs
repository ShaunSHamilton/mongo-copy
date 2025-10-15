use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::debug;

const CONFIG_FILE_NAME: &str = "config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UriEntry {
    pub name: String,
    pub uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub uris: Vec<UriEntry>,
}

impl Config {
    pub fn new() -> Self {
        Self { uris: Vec::new() }
    }

    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        debug!("Loading config from: {:?}", config_path);

        if !config_path.exists() {
            debug!("Config file does not exist, creating new config");
            return Ok(Self::new());
        }

        let content = fs::read_to_string(&config_path).context("Failed to read config file")?;

        let config: Config =
            serde_json::from_str(&content).context("Failed to parse config file")?;

        debug!("Loaded config with {} URI entries", config.uris.len());
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        debug!("Saving config to: {:?}", config_path);

        // Ensure parent directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).context("Failed to create config directory")?;
        }

        let content = serde_json::to_string_pretty(self).context("Failed to serialize config")?;

        fs::write(&config_path, content).context("Failed to write config file")?;

        debug!("Config saved successfully");
        Ok(())
    }

    pub fn add_uri(&mut self, name: String, uri: String) -> Result<()> {
        debug!("Adding URI entry: {}", name);

        // Check if name already exists
        if let Some(existing) = self.uris.iter_mut().find(|e| e.name == name) {
            debug!("Updating existing URI entry: {}", name);
            existing.uri = uri;
        } else {
            debug!("Creating new URI entry: {}", name);
            self.uris.push(UriEntry { name, uri });
        }

        self.save()?;
        Ok(())
    }

    pub fn remove_uri(&mut self, name: &str) -> Result<bool> {
        debug!("Removing URI entry: {}", name);
        let original_len = self.uris.len();
        self.uris.retain(|e| e.name != name);

        if self.uris.len() < original_len {
            self.save()?;
            debug!("URI entry removed: {}", name);
            Ok(true)
        } else {
            debug!("URI entry not found: {}", name);
            Ok(false)
        }
    }

    #[allow(dead_code)]
    pub fn get_uri(&self, name: &str) -> Option<&str> {
        self.uris
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.uri.as_str())
    }

    pub fn list_names(&self) -> Vec<String> {
        self.uris.iter().map(|e| e.name.clone()).collect()
    }

    fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().context("Failed to determine config directory")?;
        Ok(config_dir.join("mongo-copy").join(CONFIG_FILE_NAME))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}
