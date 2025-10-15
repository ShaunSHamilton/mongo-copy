mod mongo;
mod ui;

use anyhow::Result;
use clap::Parser;
use tracing::{debug, error, info, warn};

use mongo::{MongoConnection, copy_collection, copy_database};
use ui::{
    CopyMode, confirm_operation, get_copy_limit, get_destination_collection,
    get_destination_database, get_mongodb_uri, select_collections, select_copy_mode,
    select_databases, select_source_database,
};

#[derive(Parser)]
#[command(name = "mongo-copy")]
#[command(about = "Copy MongoDB databases and collections between instances", long_about = None)]
struct Cli {
    /// Source MongoDB URI (overrides MONGODB_URI_SOURCE env var)
    #[arg(long)]
    source: Option<String>,

    /// Destination MongoDB URI (overrides MONGODB_URI_DESTINATION env var)
    #[arg(long)]
    destination: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    info!("MongoDB Copy");
    debug!(
        "Parsed CLI arguments: source={:?}, destination={:?}",
        cli.source.is_some(),
        cli.destination.is_some()
    );

    // Get source URI
    let source_uri = if let Some(uri) = cli.source {
        debug!("Using source URI from CLI argument");
        uri
    } else {
        get_mongodb_uri("MONGODB_URI_SOURCE", "Enter source MongoDB URI:")?
    };

    // Get destination URI
    let dest_uri = if let Some(uri) = cli.destination {
        debug!("Using destination URI from CLI argument");
        uri
    } else {
        get_mongodb_uri("MONGODB_URI_DESTINATION", "Enter destination MongoDB URI:")?
    };

    info!("Connecting to MongoDB instances...");
    info!("Source:      {}", mask_uri(&source_uri));
    info!("Destination: {}", mask_uri(&dest_uri));
    debug!(
        "Source URI length: {}, Destination URI length: {}",
        source_uri.len(),
        dest_uri.len()
    );

    // Connect to both instances
    match MongoConnection::new(&source_uri).await {
        Ok(source) => {
            debug!("Successfully connected to source MongoDB");
            match MongoConnection::new(&dest_uri).await {
                Ok(dest) => {
                    info!("Connected successfully");
                    debug!("Both MongoDB connections established");

                    // Select copy mode
                    let mode = select_copy_mode()?;
                    debug!(
                        "Selected copy mode: {:?}",
                        match mode {
                            CopyMode::Databases => "Databases",
                            CopyMode::Collections => "Collections",
                        }
                    );

                    match mode {
                        CopyMode::Databases => {
                            handle_database_copy(&source, &dest).await?;
                        }
                        CopyMode::Collections => {
                            handle_collection_copy(&source, &dest).await?;
                        }
                    }

                    info!("All operations completed successfully!");
                    Ok(())
                }
                Err(e) => {
                    error!("Failed to connect to destination MongoDB: {}", e);
                    Err(e)
                }
            }
        }
        Err(e) => {
            error!("Failed to connect to source MongoDB: {}", e);
            Err(e)
        }
    }
}

async fn handle_database_copy(source: &MongoConnection, dest: &MongoConnection) -> Result<()> {
    let databases = select_databases(source).await?;
    debug!("Selected {} database(s) for copying", databases.len());

    for source_db in databases {
        let dest_db = get_destination_database(&source_db)?;
        debug!("Database copy: '{}' -> '{}'", source_db, dest_db);

        let operation = format!("Copy database '{}' to '{}'", source_db, dest_db);

        if !confirm_operation(&source.uri, &dest.uri, &operation)? {
            warn!(
                "Skipped database '{}' - user declined confirmation",
                source_db
            );
            info!("Skipped database '{}'", source_db);
            continue;
        }

        info!("Starting copy operation for database '{}'", source_db);
        match copy_database(source, dest, &source_db, &dest_db).await {
            Ok(_) => {
                info!("Database '{}' copied successfully", source_db);
            }
            Err(e) => {
                error!("Failed to copy database '{}': {}", source_db, e);
                return Err(e);
            }
        }
    }

    Ok(())
}

async fn handle_collection_copy(source: &MongoConnection, dest: &MongoConnection) -> Result<()> {
    let source_db = select_source_database(source).await?;
    debug!("Selected source database: '{}'", source_db);

    let collections = select_collections(source, &source_db).await?;
    debug!("Selected {} collection(s) for copying", collections.len());

    // Ask for destination database once for all collections
    let dest_db = get_destination_database(&source_db)?;
    debug!("Destination database: '{}'", dest_db);

    for source_coll in &collections {
        let dest_coll = get_destination_collection(source_coll)?;
        debug!(
            "Collection copy: '{}.{}' -> '{}.{}'",
            source_db, source_coll, dest_db, dest_coll
        );

        let limit = get_copy_limit(source, &source_db, source_coll).await?;
        debug!("Copy limit for '{}': {:?}", source_coll, limit);

        let operation = if let Some(limit_val) = limit {
            format!(
                "Copy {} documents from '{}.{}' to '{}.{}'",
                limit_val, source_db, source_coll, dest_db, dest_coll
            )
        } else {
            format!(
                "Copy all documents from '{}.{}' to '{}.{}'",
                source_db, source_coll, dest_db, dest_coll
            )
        };

        if !confirm_operation(&source.uri, &dest.uri, &operation)? {
            warn!(
                "Skipped collection '{}' - user declined confirmation",
                source_coll
            );
            info!("Skipped collection '{}'", source_coll);
            continue;
        }

        info!("Starting copy operation for collection '{}'", source_coll);
        match copy_collection(
            source,
            dest,
            &source_db,
            source_coll,
            &dest_db,
            &dest_coll,
            limit,
        )
        .await
        {
            Ok(count) => {
                info!(
                    "Copied {} documents from '{}.{}' to '{}.{}'",
                    count, source_db, source_coll, dest_db, dest_coll
                );
            }
            Err(e) => {
                error!("Failed to copy collection '{}': {}", source_coll, e);
                return Err(e);
            }
        }
    }

    Ok(())
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
