use anyhow::Result;
use inquire::{Confirm, MultiSelect, Select, Text};
use tracing::{debug, info};

use crate::config::Config;
use crate::keystore::KeyStore;
use crate::mongo::MongoConnection;

pub fn get_mongodb_uri(env_var: &str, prompt: &str, skip_env: bool) -> Result<String> {
    // Check environment variable first (unless skip_env is true)
    if !skip_env {
        if let Ok(uri) = std::env::var(env_var) {
            info!("Using {} from environment", env_var);
            return Ok(uri);
        }
    }

    // Load saved URIs from config
    let config = Config::load()?;
    let saved_names = config.list_names();

    if !saved_names.is_empty() {
        debug!("Found {} saved URI(s)", saved_names.len());

        // Add options for saved URIs and manual entry
        let mut options = saved_names.clone();
        options.push("Enter new URI manually".to_string());
        options.push("Manage saved URIs".to_string());

        let selection = Select::new(prompt, options).prompt()?;

        if selection == "Enter new URI manually" {
            prompt_and_save_uri(&config)
        } else if selection == "Manage saved URIs" {
            manage_saved_uris()
        } else {
            // Load URI from keyring
            debug!("Loading URI from keyring: {}", selection);
            if let Some(uri) = KeyStore::get_uri(&selection)? {
                info!("Using saved URI: {}", selection);
                Ok(uri)
            } else {
                info!("URI not found in keyring, prompting for manual entry");
                prompt_and_save_uri(&config)
            }
        }
    } else {
        debug!("No saved URIs found");
        prompt_and_save_uri(&config)
    }
}

fn prompt_and_save_uri(config: &Config) -> Result<String> {
    let uri = Text::new("Enter MongoDB URI:")
        .with_help_message("Example: mongodb://localhost:27017")
        .prompt()?;

    let save = Confirm::new("Save this URI for future use?")
        .with_default(true)
        .prompt()?;

    if save {
        let name = Text::new("Enter a name for this URI:")
            .with_help_message("Example: production, local, staging")
            .prompt()?;

        debug!("Saving URI with name: {}", name);
        KeyStore::store_uri(&name, &uri)?;

        let mut config = config.clone();
        config.add_uri(name.clone(), String::new())?; // Store name only in config
        info!("URI saved as: {}", name);
    }

    Ok(uri)
}

fn manage_saved_uris() -> Result<String> {
    let mut config = Config::load()?;

    loop {
        let saved_names = config.list_names();

        if saved_names.is_empty() {
            info!("No saved URIs to manage");
            // Reload config and prompt for new URI
            let config = Config::load()?;
            return prompt_and_save_uri(&config);
        }

        let mut options = vec!["← Back to URI selection".to_string()];
        options.extend(saved_names.iter().map(|name| format!("Delete: {}", name)));

        let selection = Select::new("Manage saved URIs:", options).prompt()?;

        if selection == "← Back to URI selection" {
            // Reload config and prompt for new URI
            let config = Config::load()?;
            return prompt_and_save_uri(&config);
        } else if let Some(name) = selection.strip_prefix("Delete: ") {
            let confirm = Confirm::new(&format!("Delete saved URI '{}'?", name))
                .with_default(false)
                .prompt()?;

            if confirm {
                KeyStore::delete_uri(name)?;
                config.remove_uri(name)?;
                info!("Deleted saved URI: {}", name);
            }
        }
    }
}

pub enum CopyMode {
    Databases,
    Collections,
}

pub fn select_copy_mode() -> Result<CopyMode> {
    let options = vec!["Copy entire database(s)", "Copy specific collection(s)"];
    let selection = Select::new("What would you like to copy?", options).prompt()?;

    match selection {
        "Copy entire database(s)" => Ok(CopyMode::Databases),
        "Copy specific collection(s)" => Ok(CopyMode::Collections),
        _ => unreachable!(),
    }
}

pub async fn select_databases(conn: &MongoConnection) -> Result<Vec<String>> {
    let databases = conn.list_databases().await?;

    if databases.is_empty() {
        anyhow::bail!("No databases found");
    }

    let selected = MultiSelect::new("Select database(s) to copy:", databases)
        .with_help_message("Use space to select, enter to confirm")
        .prompt()?;

    Ok(selected)
}

pub async fn select_source_database(conn: &MongoConnection) -> Result<String> {
    let databases = conn.list_databases().await?;

    if databases.is_empty() {
        anyhow::bail!("No databases found");
    }

    let selected = Select::new("Select source database:", databases).prompt()?;

    Ok(selected)
}

pub async fn select_collections(conn: &MongoConnection, database: &str) -> Result<Vec<String>> {
    let collections = conn.list_collections(database).await?;

    if collections.is_empty() {
        anyhow::bail!("No collections found in database '{}'", database);
    }

    // Build collection names with document counts
    let mut collection_options = Vec::new();
    for coll in &collections {
        let count = conn.get_collection_count(database, coll).await.unwrap_or(0);
        collection_options.push(format!("{} ({} documents)", coll, count));
    }

    let selected = MultiSelect::new(
        &format!("Select collection(s) from '{}' to copy:", database),
        collection_options,
    )
    .with_help_message("Use space to select, enter to confirm")
    .prompt()?;

    // Extract original collection names from the selected options
    let selected_names: Vec<String> = selected
        .iter()
        .map(|s| {
            // Extract the collection name before the " (" part
            s.split(" (").next().unwrap_or(s).to_string()
        })
        .collect();

    Ok(selected_names)
}

pub fn get_destination_database(source_db: &str) -> Result<String> {
    let dest_db = Text::new("Destination database name:")
        .with_default(source_db)
        .with_help_message("Press enter to use the same name, or type a new name")
        .prompt()?;
    Ok(dest_db)
}

pub fn get_destination_collection(source_coll: &str) -> Result<String> {
    let dest_coll = Text::new("Destination collection name:")
        .with_default(source_coll)
        .with_help_message("Press enter to use the same name, or type a new name")
        .prompt()?;
    Ok(dest_coll)
}

pub async fn get_copy_limit(
    conn: &MongoConnection,
    database: &str,
    collection: &str,
) -> Result<Option<u64>> {
    let count = conn.get_collection_count(database, collection).await?;

    println!(
        "Collection '{}' has approximately {} documents",
        collection, count
    );

    let copy_all = Confirm::new("Copy all documents?")
        .with_default(true)
        .prompt()?;

    if copy_all {
        Ok(None)
    } else {
        let limit_str = Text::new("How many documents to copy?")
            .with_help_message("Enter a number")
            .prompt()?;

        let limit = limit_str
            .parse::<u64>()
            .map_err(|_| anyhow::anyhow!("Invalid number"))?;

        Ok(Some(limit))
    }
}

pub fn confirm_operation(source_uri: &str, dest_uri: &str, operation: &str) -> Result<bool> {
    println!("\n{}", "=".repeat(80));
    println!("OPERATION SUMMARY");
    println!("{}", "=".repeat(80));
    println!("Source:      {}", mask_uri(source_uri));
    println!("Destination: {}", mask_uri(dest_uri));
    println!("Operation:   {}", operation);
    println!("{}", "=".repeat(80));

    let confirmed = Confirm::new("Proceed with this operation?")
        .with_default(false)
        .prompt()?;

    Ok(confirmed)
}

fn mask_uri(uri: &str) -> String {
    if let Some(at_pos) = uri.find('@') {
        if let Some(protocol_end) = uri.find("://") {
            let protocol = &uri[..protocol_end + 3];
            let after_at = &uri[at_pos..];
            return format!("{}***{}", protocol, after_at);
        }
    }
    uri.to_string()
}
