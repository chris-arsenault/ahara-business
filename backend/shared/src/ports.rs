use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawMailObject {
    pub key: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawMailMetadata {
    pub key: String,
    pub size_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundMailRequest {
    pub from_address: String,
    pub to_addresses: Vec<String>,
    pub raw_message: Vec<u8>,
}

impl OutboundMailRequest {
    pub fn validate_for_send(&self) -> AppResult<()> {
        validate_header_value("from address", &self.from_address)?;
        if self.from_address.trim().is_empty() {
            return Err(AppError::Validation("from address is required".to_string()));
        }
        if self.to_addresses.is_empty() {
            return Err(AppError::Validation(
                "at least one recipient is required".to_string(),
            ));
        }
        for address in &self.to_addresses {
            validate_header_value("recipient", address)?;
            if address.trim().is_empty() {
                return Err(AppError::Validation(
                    "recipient address is required".to_string(),
                ));
            }
        }
        if self.raw_message.is_empty() {
            return Err(AppError::Validation("raw message is required".to_string()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundMailResponse {
    pub provider_message_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackPublishRequest {
    pub topic_arn: String,
    pub payload: serde_json::Value,
}

#[async_trait]
pub trait RawMailStore: Send + Sync {
    async fn get_raw_mail_metadata(&self, key: &str) -> AppResult<RawMailMetadata>;
    async fn get_raw_mail(&self, key: &str) -> AppResult<RawMailObject>;
    async fn put_raw_mail(&self, object: RawMailObject) -> AppResult<()>;
    async fn delete_raw_mail(&self, key: &str) -> AppResult<()>;
}

#[async_trait]
pub trait MailSender: Send + Sync {
    async fn send_mail(&self, request: OutboundMailRequest) -> AppResult<OutboundMailResponse>;
}

#[async_trait]
pub trait FeedbackPublisher: Send + Sync {
    async fn publish_feedback(&self, request: FeedbackPublishRequest) -> AppResult<()>;
}

fn validate_header_value(label: &str, value: &str) -> AppResult<()> {
    if value.contains('\r') || value.contains('\n') {
        return Err(AppError::Validation(format!(
            "{label} cannot contain newlines"
        )));
    }
    Ok(())
}

#[cfg(test)]
pub mod test_doubles {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;

    use crate::error::{AppError, AppResult};

    use super::{
        FeedbackPublishRequest, FeedbackPublisher, MailSender, OutboundMailRequest,
        OutboundMailResponse, RawMailMetadata, RawMailObject, RawMailStore,
    };

    #[derive(Debug, Clone, Default)]
    pub struct InMemoryRawMailStore {
        objects: Arc<Mutex<HashMap<String, RawMailObject>>>,
        failure: Arc<Mutex<Option<String>>>,
    }

    impl InMemoryRawMailStore {
        pub fn fail_next(&self, message: impl Into<String>) {
            *self.failure.lock().unwrap() = Some(message.into());
        }

        pub fn object_count(&self) -> usize {
            self.objects.lock().unwrap().len()
        }

        fn take_failure(&self) -> AppResult<()> {
            if let Some(message) = self.failure.lock().unwrap().take() {
                return Err(AppError::ExternalService {
                    service: "raw_mail_store",
                    message,
                });
            }
            Ok(())
        }
    }

    #[async_trait]
    impl RawMailStore for InMemoryRawMailStore {
        async fn get_raw_mail_metadata(&self, key: &str) -> AppResult<RawMailMetadata> {
            self.take_failure()?;
            self.objects
                .lock()
                .unwrap()
                .get(key)
                .map(|object| RawMailMetadata {
                    key: key.to_string(),
                    size_bytes: object.bytes.len(),
                })
                .ok_or_else(|| AppError::NotFound(format!("raw mail object {key}")))
        }

        async fn get_raw_mail(&self, key: &str) -> AppResult<RawMailObject> {
            self.take_failure()?;
            self.objects
                .lock()
                .unwrap()
                .get(key)
                .cloned()
                .ok_or_else(|| AppError::NotFound(format!("raw mail object {key}")))
        }

        async fn put_raw_mail(&self, object: RawMailObject) -> AppResult<()> {
            self.take_failure()?;
            self.objects
                .lock()
                .unwrap()
                .insert(object.key.clone(), object);
            Ok(())
        }

        async fn delete_raw_mail(&self, key: &str) -> AppResult<()> {
            self.take_failure()?;
            self.objects.lock().unwrap().remove(key);
            Ok(())
        }
    }

    #[derive(Debug, Clone)]
    pub struct InMemoryMailSender {
        sent: Arc<Mutex<Vec<OutboundMailRequest>>>,
        failure: Arc<Mutex<Option<String>>>,
        next_id: Arc<Mutex<u64>>,
    }

    impl Default for InMemoryMailSender {
        fn default() -> Self {
            Self {
                sent: Arc::new(Mutex::new(Vec::new())),
                failure: Arc::new(Mutex::new(None)),
                next_id: Arc::new(Mutex::new(1)),
            }
        }
    }

    impl InMemoryMailSender {
        pub fn sent(&self) -> Vec<OutboundMailRequest> {
            self.sent.lock().unwrap().clone()
        }

        pub fn fail_next(&self, message: impl Into<String>) {
            *self.failure.lock().unwrap() = Some(message.into());
        }
    }

    #[async_trait]
    impl MailSender for InMemoryMailSender {
        async fn send_mail(&self, request: OutboundMailRequest) -> AppResult<OutboundMailResponse> {
            if let Some(message) = self.failure.lock().unwrap().take() {
                return Err(AppError::ExternalService {
                    service: "mail_sender",
                    message,
                });
            }

            self.sent.lock().unwrap().push(request);
            let mut next_id = self.next_id.lock().unwrap();
            let provider_message_id = format!("test-message-{next_id}");
            *next_id += 1;

            Ok(OutboundMailResponse {
                provider_message_id,
            })
        }
    }

    #[derive(Debug, Clone, Default)]
    pub struct InMemoryFeedbackPublisher {
        published: Arc<Mutex<Vec<FeedbackPublishRequest>>>,
        failure: Arc<Mutex<Option<String>>>,
    }

    impl InMemoryFeedbackPublisher {
        pub fn published(&self) -> Vec<FeedbackPublishRequest> {
            self.published.lock().unwrap().clone()
        }

        pub fn fail_next(&self, message: impl Into<String>) {
            *self.failure.lock().unwrap() = Some(message.into());
        }
    }

    #[async_trait]
    impl FeedbackPublisher for InMemoryFeedbackPublisher {
        async fn publish_feedback(&self, request: FeedbackPublishRequest) -> AppResult<()> {
            if let Some(message) = self.failure.lock().unwrap().take() {
                return Err(AppError::ExternalService {
                    service: "feedback_publisher",
                    message,
                });
            }

            self.published.lock().unwrap().push(request);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::test_doubles::{
        InMemoryFeedbackPublisher, InMemoryMailSender, InMemoryRawMailStore,
    };
    use super::{
        FeedbackPublishRequest, FeedbackPublisher, MailSender, OutboundMailRequest, RawMailObject,
        RawMailStore,
    };

    #[tokio::test]
    async fn in_memory_raw_mail_store_records_objects_and_simulates_failure() {
        let store = InMemoryRawMailStore::default();
        store
            .put_raw_mail(RawMailObject {
                key: "raw/message".to_string(),
                bytes: b"message".to_vec(),
            })
            .await
            .unwrap();

        assert_eq!(store.object_count(), 1);
        assert_eq!(
            store
                .get_raw_mail_metadata("raw/message")
                .await
                .unwrap()
                .size_bytes,
            b"message".len()
        );
        assert_eq!(
            store.get_raw_mail("raw/message").await.unwrap().bytes,
            b"message"
        );

        store.fail_next("unavailable");
        assert!(store.get_raw_mail("raw/message").await.is_err());
    }

    #[tokio::test]
    async fn in_memory_mail_sender_records_requests_and_simulates_failure() {
        let sender = InMemoryMailSender::default();
        let response = sender
            .send_mail(OutboundMailRequest {
                from_address: "contact@ahara.io".to_string(),
                to_addresses: vec!["recipient@example.test".to_string()],
                raw_message: b"body".to_vec(),
            })
            .await
            .unwrap();

        assert_eq!(response.provider_message_id, "test-message-1");
        assert_eq!(sender.sent().len(), 1);

        sender.fail_next("throttled");
        assert!(
            sender
                .send_mail(OutboundMailRequest {
                    from_address: "contact@ahara.io".to_string(),
                    to_addresses: vec!["recipient@example.test".to_string()],
                    raw_message: b"body".to_vec(),
                })
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn in_memory_feedback_publisher_records_requests_and_simulates_failure() {
        let publisher = InMemoryFeedbackPublisher::default();
        publisher
            .publish_feedback(FeedbackPublishRequest {
                topic_arn: "arn:aws:sns:::topic".to_string(),
                payload: json!({ "kind": "bounce" }),
            })
            .await
            .unwrap();

        assert_eq!(publisher.published().len(), 1);

        publisher.fail_next("publish failed");
        assert!(
            publisher
                .publish_feedback(FeedbackPublishRequest {
                    topic_arn: "arn:aws:sns:::topic".to_string(),
                    payload: json!({ "kind": "complaint" }),
                })
                .await
                .is_err()
        );
    }
}
