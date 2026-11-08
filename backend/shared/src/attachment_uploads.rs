use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::attachment_storage::{AttachmentStorage, SignedAttachmentUrl, StoredAttachment};
use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::outbound::{
    MAX_OUTBOUND_ATTACHMENT_BYTES, MAX_OUTBOUND_ATTACHMENT_COUNT, OutboundAttachmentInput,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateAttachmentUpload {
    pub filename: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub checksum_sha256: String,
}

impl CreateAttachmentUpload {
    pub fn validate(&self) -> AppResult<()> {
        if self.size_bytes < 1 || self.size_bytes > MAX_OUTBOUND_ATTACHMENT_BYTES as i64 {
            return Err(AppError::Validation(
                "attachments must be between 1 byte and 15 MB (15 MiB)".into(),
            ));
        }
        if self.filename.is_empty()
            || self.filename.len() > 255
            || self
                .filename
                .chars()
                .any(|c| c.is_control() || c == '/' || c == '\\')
        {
            return Err(AppError::Validation("invalid attachment filename".into()));
        }
        if self.content_type.is_empty()
            || self.content_type.len() > 255
            || !self.content_type.is_ascii()
            || self.content_type.chars().any(char::is_whitespace)
            || !self.content_type.contains('/')
        {
            return Err(AppError::Validation(
                "invalid attachment content type".into(),
            ));
        }
        if base64::engine::general_purpose::STANDARD
            .decode(&self.checksum_sha256)
            .map(|v| v.len())
            .ok()
            != Some(32)
        {
            return Err(AppError::Validation(
                "checksum_sha256 must be a base64 SHA-256 digest".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UploadedAttachmentRef {
    pub upload_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentUploadGrant {
    pub upload_id: Uuid,
    pub upload: SignedAttachmentUrl,
}

#[async_trait]
pub trait AttachmentUploadService: Send + Sync {
    async fn create(
        &self,
        owner: &str,
        request: CreateAttachmentUpload,
    ) -> AppResult<AttachmentUploadGrant>;
    async fn resolve(
        &self,
        owner: &str,
        references: &[UploadedAttachmentRef],
    ) -> AppResult<Vec<OutboundAttachmentInput>>;
}

#[derive(Clone)]
pub struct PgAttachmentUploadService {
    pool: DbPool,
    storage: Arc<dyn AttachmentStorage>,
}

impl PgAttachmentUploadService {
    pub fn new(pool: DbPool, storage: Arc<dyn AttachmentStorage>) -> Self {
        Self { pool, storage }
    }
}

#[async_trait]
impl AttachmentUploadService for PgAttachmentUploadService {
    async fn create(
        &self,
        owner: &str,
        request: CreateAttachmentUpload,
    ) -> AppResult<AttachmentUploadGrant> {
        request.validate()?;
        let upload_id = Uuid::new_v4();
        let upload = self
            .storage
            .grant_upload(&staging_key(upload_id), &request)
            .await?;
        sqlx::query("INSERT INTO attachment_uploads (id, owner_sub, filename, content_type, size_bytes, checksum_sha256) VALUES ($1,$2,$3,$4,$5,$6)")
            .bind(upload_id).bind(owner).bind(&request.filename).bind(&request.content_type)
            .bind(request.size_bytes).bind(&request.checksum_sha256).execute(&self.pool).await.map_err(db_error)?;
        Ok(AttachmentUploadGrant { upload_id, upload })
    }

    async fn resolve(
        &self,
        owner: &str,
        references: &[UploadedAttachmentRef],
    ) -> AppResult<Vec<OutboundAttachmentInput>> {
        if references.len() > MAX_OUTBOUND_ATTACHMENT_COUNT {
            return Err(AppError::Validation(
                "at most 10 attachments are allowed".into(),
            ));
        }
        let mut tx = self.pool.begin().await.map_err(db_error)?;
        let mut rows = Vec::new();
        let mut total = 0i64;
        for reference in references {
            let row: UploadRow = sqlx::query_as("SELECT filename, content_type, size_bytes, checksum_sha256, object_key, version_id FROM attachment_uploads WHERE id = $1 AND owner_sub = $2 AND expires_at > now() FOR UPDATE")
                .bind(reference.upload_id).bind(owner).fetch_optional(&mut *tx).await.map_err(db_error)?
                .ok_or_else(|| AppError::Validation("attachment upload is missing, expired or belongs to another user; attach the file again".into()))?;
            total += row.size_bytes;
            if total > MAX_OUTBOUND_ATTACHMENT_BYTES as i64 {
                return Err(AppError::Validation(
                    "attachments cannot exceed 15 MB (15 MiB) combined".into(),
                ));
            }
            rows.push((reference.upload_id, row));
        }
        let mut resolved = Vec::new();
        for (id, row) in rows {
            let object = match (row.object_key, row.version_id) {
                (Some(key), Some(version_id)) => StoredAttachment {
                    key,
                    version_id,
                    size_bytes: row.size_bytes,
                },
                _ => {
                    let object = self
                        .storage
                        .finalize_upload(
                            &staging_key(id),
                            &CreateAttachmentUpload {
                                filename: row.filename.clone(),
                                content_type: row.content_type.clone(),
                                size_bytes: row.size_bytes,
                                checksum_sha256: row.checksum_sha256,
                            },
                        )
                        .await?;
                    sqlx::query("UPDATE attachment_uploads SET object_key = $2, version_id = $3 WHERE id = $1")
                        .bind(id).bind(&object.key).bind(&object.version_id).execute(&mut *tx).await.map_err(db_error)?;
                    object
                }
            };
            resolved.push(OutboundAttachmentInput {
                filename: row.filename,
                content_type: row.content_type,
                content_base64: String::new(),
                storage: Some(object),
            });
        }
        tx.commit().await.map_err(db_error)?;
        Ok(resolved)
    }
}

fn staging_key(id: Uuid) -> String {
    format!("attachments/staging/{id}")
}
fn db_error(err: sqlx::Error) -> AppError {
    AppError::Database(err.to_string())
}

#[derive(sqlx::FromRow)]
struct UploadRow {
    filename: String,
    content_type: String,
    size_bytes: i64,
    checksum_sha256: String,
    object_key: Option<String>,
    version_id: Option<String>,
}
