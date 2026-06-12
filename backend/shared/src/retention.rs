use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::ports::RawMailStore;

pub const RAW_RETENTION_CLEANUP_LIMIT: i64 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RawMailRetentionSummary {
    pub candidates: usize,
    pub deleted: usize,
    pub failed: usize,
}

#[async_trait]
pub trait RawMailRetentionService: Send + Sync {
    async fn cleanup_due_raw_mail(&self) -> AppResult<RawMailRetentionSummary>;
}

#[derive(Clone)]
pub struct PgRawMailRetentionService {
    pool: DbPool,
    raw_mail_store: Arc<dyn RawMailStore>,
    limit: i64,
}

impl PgRawMailRetentionService {
    pub fn new(pool: DbPool, raw_mail_store: Arc<dyn RawMailStore>) -> Self {
        Self {
            pool,
            raw_mail_store,
            limit: RAW_RETENTION_CLEANUP_LIMIT,
        }
    }
}

#[async_trait]
impl RawMailRetentionService for PgRawMailRetentionService {
    async fn cleanup_due_raw_mail(&self) -> AppResult<RawMailRetentionSummary> {
        let candidates: Vec<RetentionCandidateRow> = sqlx::query_as(
            "SELECT id, s3_raw_key
             FROM messages
             WHERE direction = 'inbound'
               AND s3_raw_key IS NOT NULL
               AND raw_deleted_at IS NULL
               AND raw_retained_until IS NOT NULL
               AND raw_retained_until <= now()
             ORDER BY raw_retained_until ASC, id ASC
             LIMIT $1",
        )
        .bind(self.limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        let mut summary = RawMailRetentionSummary {
            candidates: candidates.len(),
            ..RawMailRetentionSummary::default()
        };
        for candidate in candidates {
            let Some(key) = candidate.s3_raw_key else {
                continue;
            };
            match self.raw_mail_store.delete_raw_mail(&key).await {
                Ok(()) => {
                    self.mark_deleted(candidate.id).await?;
                    summary.deleted += 1;
                }
                Err(err) => {
                    summary.failed += 1;
                    tracing::warn!(
                        message_id = %candidate.id,
                        error = %err,
                        "raw mail retention delete failed"
                    );
                }
            }
        }
        Ok(summary)
    }
}

impl PgRawMailRetentionService {
    async fn mark_deleted(&self, message_id: Uuid) -> AppResult<()> {
        sqlx::query(
            "UPDATE messages
             SET raw_deleted_at = now(),
                 updated_at = now()
             WHERE id = $1
               AND raw_deleted_at IS NULL",
        )
        .bind(message_id)
        .execute(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;
        Ok(())
    }
}

#[derive(Debug, sqlx::FromRow)]
struct RetentionCandidateRow {
    id: Uuid,
    s3_raw_key: Option<String>,
}
