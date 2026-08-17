use anyhow::Result;
use bson::{doc, DateTime as BsonDateTime};
use chrono::Utc;
use mongodb::{options::UpdateOptions, Collection, Database};

/// Registry that records worker liveness so the API can report it
pub struct WorkerRegistry {
    collection: Collection<bson::Document>,
    worker_id: String,
}

impl WorkerRegistry {
    pub fn new(db: &Database, worker_id: String) -> Self {
        Self {
            collection: db.collection("workers"),
            worker_id,
        }
    }

    /// Record that this worker is alive and what it is doing
    pub async fn heartbeat(&self, current_job: Option<&str>) -> Result<()> {
        let now = BsonDateTime::from_chrono(Utc::now());

        self.collection
            .update_one(
                doc! { "worker_id": &self.worker_id },
                doc! {
                    "$set": {
                        "last_seen_at": now,
                        "current_job_id": current_job,
                    },
                    "$setOnInsert": { "started_at": now },
                },
            )
            .with_options(UpdateOptions::builder().upsert(true).build())
            .await?;

        Ok(())
    }

    /// Remove this worker's registration on clean shutdown
    pub async fn deregister(&self) -> Result<()> {
        self.collection
            .delete_one(doc! { "worker_id": &self.worker_id })
            .await?;

        Ok(())
    }
}
