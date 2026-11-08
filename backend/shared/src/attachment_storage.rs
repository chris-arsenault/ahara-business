use std::{collections::BTreeMap, time::Duration};

use async_trait::async_trait;
use aws_sdk_s3::{Client, presigning::PresigningConfig, primitives::ByteStream};
use serde::{Deserialize, Serialize};

use crate::attachment_uploads::CreateAttachmentUpload;
#[cfg(test)]
mod tests;
use crate::{
    config::MailConfig,
    error::{AppError, AppResult},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredAttachment {
    pub key: String,
    pub version_id: String,
    pub size_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedAttachmentUrl {
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub expires_in_seconds: u64,
}

#[async_trait]
pub trait AttachmentStorage: Send + Sync {
    async fn grant_upload(
        &self,
        key: &str,
        request: &CreateAttachmentUpload,
    ) -> AppResult<SignedAttachmentUrl>;
    async fn finalize_upload(
        &self,
        key: &str,
        request: &CreateAttachmentUpload,
    ) -> AppResult<StoredAttachment>;
    async fn read(&self, object: &StoredAttachment) -> AppResult<Vec<u8>>;
    async fn download(
        &self,
        object: &StoredAttachment,
        filename: &str,
    ) -> AppResult<SignedAttachmentUrl>;
    async fn cache_download(
        &self,
        key: &str,
        bytes: Vec<u8>,
        filename: &str,
    ) -> AppResult<SignedAttachmentUrl>;
}

#[derive(Debug, Clone)]
pub struct S3AttachmentStorage {
    client: Client,
    bucket: String,
}

impl S3AttachmentStorage {
    pub fn new(client: Client, bucket: String) -> Self {
        Self { client, bucket }
    }

    pub async fn from_env(config: &MailConfig) -> Self {
        let sdk = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        Self::new(Client::new(&sdk), config.raw_mail_bucket.clone())
    }
}

fn signing_config() -> AppResult<PresigningConfig> {
    PresigningConfig::expires_in(Duration::from_secs(900)).map_err(storage_error)
}

fn storage_error(error: impl std::fmt::Display) -> AppError {
    AppError::ExternalService {
        service: "s3",
        message: error.to_string(),
    }
}

fn validate_key(key: &str) -> AppResult<()> {
    if key.starts_with("attachments/") && !key.contains("..") {
        Ok(())
    } else {
        Err(AppError::Validation("invalid attachment object key".into()))
    }
}

#[async_trait]
impl AttachmentStorage for S3AttachmentStorage {
    async fn grant_upload(
        &self,
        key: &str,
        request: &CreateAttachmentUpload,
    ) -> AppResult<SignedAttachmentUrl> {
        validate_key(key)?;
        let signed = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type(&request.content_type)
            .content_length(request.size_bytes)
            .checksum_sha256(&request.checksum_sha256)
            .presigned(signing_config()?)
            .await
            .map_err(storage_error)?;
        // Browsers supply Content-Length themselves. All other signed headers must
        // be sent exactly; never include the API's authorization header here.
        let headers = signed
            .headers()
            .filter(|(name, _)| !matches!(*name, "host" | "content-length"))
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();
        Ok(SignedAttachmentUrl {
            url: signed.uri().to_string(),
            headers,
            expires_in_seconds: 900,
        })
    }

    async fn finalize_upload(
        &self,
        key: &str,
        request: &CreateAttachmentUpload,
    ) -> AppResult<StoredAttachment> {
        validate_key(key)?;
        let head = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .checksum_mode(aws_sdk_s3::types::ChecksumMode::Enabled)
            .send()
            .await
            .map_err(storage_error)?;
        if head.content_length() != Some(request.size_bytes)
            || head.checksum_sha256() != Some(request.checksum_sha256.as_str())
            || head.content_type() != Some(request.content_type.as_str())
        {
            return Err(AppError::Validation(
                "uploaded attachment does not match its declared size, checksum or type".into(),
            ));
        }
        let version = head.version_id().filter(|v| *v != "null").ok_or_else(|| {
            AppError::Internal("attachment bucket must have versioning enabled".into())
        })?;
        let destination = format!("attachments/files/{}", uuid::Uuid::new_v4());
        let copy = self
            .client
            .copy_object()
            .bucket(&self.bucket)
            .key(&destination)
            .copy_source(format!(
                "{}/{key}?versionId={}",
                self.bucket,
                percent_encode(version)
            ))
            .send()
            .await
            .map_err(storage_error)?;
        let version_id = copy
            .version_id()
            .ok_or_else(|| AppError::Internal("copied attachment has no version".into()))?;
        Ok(StoredAttachment {
            key: destination,
            version_id: version_id.into(),
            size_bytes: request.size_bytes,
        })
    }

    async fn read(&self, object: &StoredAttachment) -> AppResult<Vec<u8>> {
        validate_key(&object.key)?;
        let response = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&object.key)
            .version_id(&object.version_id)
            .send()
            .await
            .map_err(storage_error)?;
        if response.content_length() != Some(object.size_bytes) {
            return Err(AppError::Internal("stored attachment size changed".into()));
        }
        let bytes = response
            .body
            .collect()
            .await
            .map_err(storage_error)?
            .into_bytes()
            .to_vec();
        if bytes.len() as i64 != object.size_bytes {
            return Err(AppError::Internal("incomplete attachment read".into()));
        }
        Ok(bytes)
    }

    async fn download(
        &self,
        object: &StoredAttachment,
        filename: &str,
    ) -> AppResult<SignedAttachmentUrl> {
        validate_key(&object.key)?;
        let filename = crate::mailbox::sanitize_attachment_filename(filename);
        let signed = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&object.key)
            .version_id(&object.version_id)
            .response_content_type("application/octet-stream")
            .response_content_disposition(format!(
                "attachment; filename*=UTF-8''{}",
                percent_encode(&filename)
            ))
            .presigned(signing_config()?)
            .await
            .map_err(storage_error)?;
        Ok(SignedAttachmentUrl {
            url: signed.uri().to_string(),
            headers: BTreeMap::new(),
            expires_in_seconds: 900,
        })
    }

    async fn cache_download(
        &self,
        key: &str,
        bytes: Vec<u8>,
        filename: &str,
    ) -> AppResult<SignedAttachmentUrl> {
        validate_key(key)?;
        let size_bytes = bytes.len() as i64;
        let result = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type("application/octet-stream")
            .body(ByteStream::from(bytes))
            .send()
            .await
            .map_err(storage_error)?;
        let version_id = result
            .version_id()
            .ok_or_else(|| AppError::Internal("attachment bucket must be versioned".into()))?;
        self.download(
            &StoredAttachment {
                key: key.into(),
                version_id: version_id.into(),
                size_bytes,
            },
            filename,
        )
        .await
    }
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
