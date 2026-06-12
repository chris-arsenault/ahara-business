use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_s3::Client;
use aws_sdk_s3::primitives::ByteStream;

use crate::config::MailConfig;
use crate::error::{AppError, AppResult};
use crate::ports::{RawMailMetadata, RawMailObject, RawMailStore};

#[derive(Debug, Clone)]
pub struct S3RawMailStore {
    client: Client,
    bucket: String,
    raw_mail_prefix: String,
}

impl S3RawMailStore {
    pub fn new(
        client: Client,
        bucket: impl Into<String>,
        raw_mail_prefix: impl Into<String>,
    ) -> Self {
        Self {
            client,
            bucket: bucket.into(),
            raw_mail_prefix: raw_mail_prefix.into(),
        }
    }

    pub fn from_mail_config(client: Client, config: &MailConfig) -> Self {
        Self::new(
            client,
            config.raw_mail_bucket.clone(),
            config.raw_mail_prefix.clone(),
        )
    }

    pub async fn from_env(config: &MailConfig) -> Self {
        let sdk_config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        Self::from_mail_config(Client::new(&sdk_config), config)
    }

    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    pub fn raw_mail_prefix(&self) -> &str {
        &self.raw_mail_prefix
    }

    fn validate_key(&self, key: &str) -> AppResult<String> {
        let key = key.trim();
        if key.is_empty() {
            return Err(AppError::Validation(
                "raw mail object key is required".to_string(),
            ));
        }
        if !self.raw_mail_prefix.is_empty() && !key.starts_with(&self.raw_mail_prefix) {
            return Err(AppError::Validation(
                "raw mail object key is outside configured prefix".to_string(),
            ));
        }
        Ok(key.to_string())
    }
}

#[async_trait]
impl RawMailStore for S3RawMailStore {
    async fn get_raw_mail_metadata(&self, key: &str) -> AppResult<RawMailMetadata> {
        let key = self.validate_key(key)?;
        let output = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|err| AppError::ExternalService {
                service: "s3",
                message: err.to_string(),
            })?;
        let content_length = output.content_length().unwrap_or_default();
        let size_bytes =
            usize::try_from(content_length).map_err(|_| AppError::ExternalService {
                service: "s3",
                message: "raw mail object content length is invalid".to_string(),
            })?;

        Ok(RawMailMetadata { key, size_bytes })
    }

    async fn get_raw_mail(&self, key: &str) -> AppResult<RawMailObject> {
        let key = self.validate_key(key)?;
        let output = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|err| AppError::ExternalService {
                service: "s3",
                message: err.to_string(),
            })?;
        let bytes = output
            .body
            .collect()
            .await
            .map_err(|err| AppError::ExternalService {
                service: "s3",
                message: err.to_string(),
            })?
            .into_bytes()
            .to_vec();

        Ok(RawMailObject { key, bytes })
    }

    async fn put_raw_mail(&self, object: RawMailObject) -> AppResult<()> {
        let key = self.validate_key(&object.key)?;
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(object.bytes))
            .send()
            .await
            .map_err(|err| AppError::ExternalService {
                service: "s3",
                message: err.to_string(),
            })?;
        Ok(())
    }

    async fn delete_raw_mail(&self, key: &str) -> AppResult<()> {
        let key = self.validate_key(key)?;
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|err| AppError::ExternalService {
                service: "s3",
                message: err.to_string(),
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use aws_config::Region;
    use aws_sdk_s3::config::{BehaviorVersion, Credentials};

    use crate::config::MailConfig;
    use crate::error::AppError;

    use super::S3RawMailStore;

    fn client() -> aws_sdk_s3::Client {
        let config = aws_sdk_s3::Config::builder()
            .region(Region::new("us-east-1"))
            .behavior_version(BehaviorVersion::latest())
            .credentials_provider(Credentials::new("test", "test", None, None, "unit-test"))
            .build();
        aws_sdk_s3::Client::from_conf(config)
    }

    #[test]
    fn raw_mail_store_constructs_from_mail_config() {
        let config = MailConfig {
            domain: "ahara.io".to_string(),
            raw_mail_bucket: "ahara-business-raw-mail-test".to_string(),
            raw_mail_prefix: "raw/".to_string(),
        };
        let store = S3RawMailStore::from_mail_config(client(), &config);

        assert_eq!(store.bucket(), "ahara-business-raw-mail-test");
        assert_eq!(store.raw_mail_prefix(), "raw/");
    }

    #[test]
    fn raw_mail_store_enforces_configured_prefix() {
        let store = S3RawMailStore::new(client(), "bucket", "raw/inbound/");

        assert_eq!(
            store.validate_key("raw/inbound/ses-message-1").unwrap(),
            "raw/inbound/ses-message-1"
        );
        assert!(matches!(
            store.validate_key("raw/outbound/ses-message-1"),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            store.validate_key(""),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn raw_mail_store_external_errors_remain_public_safe() {
        let err = AppError::ExternalService {
            service: "s3",
            message: "bucket contained sender@example.test and raw bytes".to_string(),
        };

        assert_eq!(err.public_message(), "internal error");
    }
}
