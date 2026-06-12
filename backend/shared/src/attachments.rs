use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::Engine;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::inbound::limits::IngestLimits;
use crate::inbound::mime::extract_attachment_body;
use crate::mailbox::sanitize_attachment_filename;
use crate::ports::RawMailStore;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailboxAttachmentDownload {
    pub id: String,
    pub message_id: String,
    pub filename: String,
    pub display_filename: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub content_id: Option<String>,
    pub content_base64: String,
}

#[async_trait]
pub trait AttachmentService: Send + Sync {
    async fn download_attachment(
        &self,
        message_id: &str,
        attachment_id: &str,
    ) -> AppResult<MailboxAttachmentDownload>;
}

#[derive(Clone)]
pub struct PgAttachmentService {
    pool: DbPool,
    raw_mail_store: Arc<dyn RawMailStore>,
    limits: IngestLimits,
}

impl PgAttachmentService {
    pub fn new(pool: DbPool, raw_mail_store: Arc<dyn RawMailStore>, limits: IngestLimits) -> Self {
        Self {
            pool,
            raw_mail_store,
            limits,
        }
    }
}

#[async_trait]
impl AttachmentService for PgAttachmentService {
    async fn download_attachment(
        &self,
        message_id: &str,
        attachment_id: &str,
    ) -> AppResult<MailboxAttachmentDownload> {
        let message_id = parse_uuid(message_id, "message id")?;
        let attachment_id = parse_uuid(attachment_id, "attachment id")?;
        let row: AttachmentDownloadRow = sqlx::query_as(
            "SELECT messages.id AS message_id,
                    messages.s3_raw_key,
                    attachment_refs.id AS attachment_id,
                    attachment_refs.position,
                    attachment_refs.filename,
                    attachment_refs.content_type,
                    attachment_refs.content_id
             FROM messages
             JOIN attachment_refs ON attachment_refs.message_id = messages.id
             WHERE messages.id = $1
               AND attachment_refs.id = $2
               AND messages.direction = 'inbound'
               AND messages.status = 'received'
               AND messages.security_disposition = 'accepted'
               AND messages.s3_raw_key IS NOT NULL
               AND messages.raw_deleted_at IS NULL",
        )
        .bind(message_id)
        .bind(attachment_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("attachment {attachment_id}")))?;

        let raw_key = row
            .s3_raw_key
            .ok_or_else(|| AppError::NotFound(format!("raw mail for message {message_id}")))?;
        let raw = self.raw_mail_store.get_raw_mail(&raw_key).await?;
        let body = extract_attachment_body(&raw.bytes, row.position, self.limits)
            .map_err(|err| AppError::Validation(err.to_string()))?
            .ok_or_else(|| AppError::NotFound(format!("attachment {attachment_id}")))?;

        Ok(MailboxAttachmentDownload {
            id: row.attachment_id.to_string(),
            message_id: row.message_id.to_string(),
            filename: row.filename,
            display_filename: sanitize_attachment_filename(&body.filename),
            content_type: row.content_type,
            size_bytes: body.bytes.len() as i64,
            content_id: row.content_id,
            content_base64: base64::engine::general_purpose::STANDARD.encode(body.bytes),
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct AttachmentDownloadRow {
    message_id: Uuid,
    s3_raw_key: Option<String>,
    attachment_id: Uuid,
    position: i32,
    filename: String,
    content_type: String,
    content_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryAttachmentService {
    downloads: Arc<Mutex<AttachmentDownloadMap>>,
}

type AttachmentDownloadMap = BTreeMap<(String, String), MailboxAttachmentDownload>;

impl InMemoryAttachmentService {
    pub fn with_downloads(downloads: impl IntoIterator<Item = MailboxAttachmentDownload>) -> Self {
        let downloads = downloads
            .into_iter()
            .map(|download| ((download.message_id.clone(), download.id.clone()), download))
            .collect();
        Self {
            downloads: Arc::new(Mutex::new(downloads)),
        }
    }
}

#[async_trait]
impl AttachmentService for InMemoryAttachmentService {
    async fn download_attachment(
        &self,
        message_id: &str,
        attachment_id: &str,
    ) -> AppResult<MailboxAttachmentDownload> {
        self.downloads
            .lock()
            .unwrap()
            .get(&(message_id.to_string(), attachment_id.to_string()))
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("attachment {attachment_id}")))
    }
}

fn parse_uuid(value: &str, label: &str) -> AppResult<Uuid> {
    Uuid::parse_str(value).map_err(|_| AppError::Validation(format!("{label} must be a UUID")))
}

#[cfg(test)]
mod tests {
    use super::{AttachmentService, InMemoryAttachmentService, MailboxAttachmentDownload};

    #[tokio::test]
    async fn in_memory_attachment_service_returns_seeded_downloads() {
        let service = InMemoryAttachmentService::with_downloads([MailboxAttachmentDownload {
            id: "attachment-1".to_string(),
            message_id: "message-1".to_string(),
            filename: "../invoice.pdf".to_string(),
            display_filename: "invoice.pdf".to_string(),
            content_type: "application/pdf".to_string(),
            size_bytes: 4,
            content_id: None,
            content_base64: "ZGF0YQ==".to_string(),
        }]);

        let download = service
            .download_attachment("message-1", "attachment-1")
            .await
            .unwrap();

        assert_eq!(download.display_filename, "invoice.pdf");
        assert_eq!(download.content_base64, "ZGF0YQ==");
    }
}
