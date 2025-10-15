use anyhow::{Context, Result};
use futures::stream::TryStreamExt;
use mongodb::{
    bson::{doc, Document},
    options::ClientOptions,
    Client, Database,
};
use tracing::{debug, error, info, warn};

pub struct MongoConnection {
    pub client: Client,
    pub uri: String,
}

impl MongoConnection {
    pub async fn new(uri: &str) -> Result<Self> {
        debug!("Parsing MongoDB URI");
        let mut client_options = ClientOptions::parse(uri)
            .await
            .context("Failed to parse MongoDB URI")?;
        client_options.app_name = Some("mongo-copy".to_string());
        debug!("MongoDB client options configured: app_name=mongo-copy");

        debug!("Creating MongoDB client");
        let client =
            Client::with_options(client_options).context("Failed to create MongoDB client")?;

        // Test connection
        debug!("Testing MongoDB connection by listing databases");
        match client.list_database_names().await {
            Ok(_) => {
                debug!("MongoDB connection test successful");
            }
            Err(e) => {
                match e.kind.as_ref() {
                    mongodb::error::ErrorKind::ServerSelection { message, .. } => {
                        info!("Likely, the URI needs to include the `directConnection=true` parameter.");
                    }
                    _ => {
                        error!("MongoDB connection test failed: {}", e);
                    }
                }
                return Err(e).context("Failed to connect to MongoDB");
            }
        }

        Ok(Self {
            client,
            uri: uri.to_string(),
        })
    }

    pub async fn list_databases(&self) -> Result<Vec<String>> {
        debug!("Listing databases");
        let databases = self.client.list_database_names().await?;
        debug!("Found {} databases", databases.len());
        Ok(databases)
    }

    pub async fn list_collections(&self, database_name: &str) -> Result<Vec<String>> {
        debug!("Listing collections in database '{}'", database_name);
        let db = self.client.database(database_name);
        let collections = db.list_collection_names().await?;
        debug!(
            "Found {} collections in database '{}'",
            collections.len(),
            database_name
        );
        Ok(collections)
    }

    pub fn get_database(&self, name: &str) -> Database {
        debug!("Getting database handle for '{}'", name);
        self.client.database(name)
    }

    pub async fn get_collection_count(&self, database: &str, collection: &str) -> Result<u64> {
        debug!("Getting document count for '{}.{}'", database, collection);
        let db = self.client.database(database);
        let coll = db.collection::<Document>(collection);
        match coll.estimated_document_count().await {
            Ok(count) => {
                debug!(
                    "Collection '{}.{}' has approximately {} documents",
                    database, collection, count
                );
                Ok(count)
            }
            Err(e) => {
                warn!(
                    "Failed to get count for '{}.{}': {}",
                    database, collection, e
                );
                Err(e.into())
            }
        }
    }
}

pub async fn copy_collection(
    source: &MongoConnection,
    dest: &MongoConnection,
    source_db: &str,
    source_coll: &str,
    dest_db: &str,
    dest_coll: &str,
    limit: Option<u64>,
) -> Result<u64> {
    debug!(
        "Starting collection copy: '{}.{}' -> '{}.{}' (limit: {:?})",
        source_db, source_coll, dest_db, dest_coll, limit
    );

    let source_collection = source
        .get_database(source_db)
        .collection::<Document>(source_coll);

    let dest_collection = dest.get_database(dest_db).collection::<Document>(dest_coll);

    debug!("Creating cursor for source collection");
    let mut cursor = if let Some(limit_val) = limit {
        debug!("Applying limit of {} documents", limit_val);
        source_collection
            .find(doc! {})
            .limit(limit_val as i64)
            .await?
    } else {
        debug!("No limit applied, copying all documents");
        source_collection.find(doc! {}).await?
    };

    let mut count = 0u64;
    let mut batch = Vec::new();
    const BATCH_SIZE: usize = 1000;
    debug!("Using batch size of {} documents", BATCH_SIZE);

    while let Some(doc) = cursor.try_next().await? {
        batch.push(doc);
        count += 1;

        if batch.len() >= BATCH_SIZE {
            debug!("Inserting batch of {} documents", batch.len());
            match dest_collection.insert_many(&batch).await {
                Ok(_) => {
                    info!("  Copied {} documents...", count);
                    batch.clear();
                }
                Err(e) => {
                    error!("Failed to insert batch at document {}: {}", count, e);
                    return Err(e.into());
                }
            }
        }
    }

    if !batch.is_empty() {
        debug!("Inserting final batch of {} documents", batch.len());
        match dest_collection.insert_many(&batch).await {
            Ok(_) => {
                debug!("Final batch inserted successfully");
            }
            Err(e) => {
                error!("Failed to insert final batch: {}", e);
                return Err(e.into());
            }
        }
    }

    debug!("Collection copy completed: {} total documents", count);
    Ok(count)
}

pub async fn copy_database(
    source: &MongoConnection,
    dest: &MongoConnection,
    source_db: &str,
    dest_db: &str,
) -> Result<()> {
    debug!("Starting database copy: '{}' -> '{}'", source_db, dest_db);
    let collections = source.list_collections(source_db).await?;

    info!("Copying database '{}' to '{}'", source_db, dest_db);
    info!("Found {} collections", collections.len());

    for (idx, collection) in collections.iter().enumerate() {
        info!(
            "\nCopying collection '{}' ({}/{})",
            collection,
            idx + 1,
            collections.len()
        );
        debug!("Collection: '{}.{}'", source_db, collection);

        match copy_collection(
            source, dest, source_db, collection, dest_db, collection, None,
        )
        .await
        {
            Ok(count) => {
                info!("Copied {} documents from '{}'", count, collection);
            }
            Err(e) => {
                error!("Failed to copy collection '{}': {}", collection, e);
                return Err(e);
            }
        }
    }

    debug!("Database copy completed successfully");
    Ok(())
}
