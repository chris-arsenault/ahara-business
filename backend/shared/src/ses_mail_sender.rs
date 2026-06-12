use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_ses::Client;
use aws_sdk_ses::primitives::Blob;
use aws_sdk_ses::types::RawMessage;

use crate::error::{AppError, AppResult};
use crate::ports::{MailSender, OutboundMailRequest, OutboundMailResponse};

#[derive(Debug, Clone)]
pub struct SesMailSender {
    client: Client,
}

impl SesMailSender {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn from_env() -> Self {
        let sdk_config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        Self::new(Client::new(&sdk_config))
    }

    fn raw_message(bytes: Vec<u8>) -> AppResult<RawMessage> {
        RawMessage::builder()
            .data(Blob::new(bytes))
            .build()
            .map_err(|err| AppError::Internal(format!("failed to build SES raw message: {err}")))
    }
}

#[async_trait]
impl MailSender for SesMailSender {
    async fn send_mail(&self, request: OutboundMailRequest) -> AppResult<OutboundMailResponse> {
        request.validate_for_send()?;
        let raw_message = Self::raw_message(request.raw_message)?;
        let output = self
            .client
            .send_raw_email()
            .source(request.from_address)
            .set_destinations(Some(request.to_addresses))
            .raw_message(raw_message)
            .send()
            .await
            .map_err(|err| AppError::ExternalService {
                service: "ses",
                message: err.to_string(),
            })?;

        Ok(OutboundMailResponse {
            provider_message_id: output.message_id().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use aws_config::Region;
    use aws_sdk_ses::config::{BehaviorVersion, Credentials};

    use crate::error::AppError;
    use crate::ports::{MailSender, OutboundMailRequest};

    use super::SesMailSender;

    fn client() -> aws_sdk_ses::Client {
        let config = aws_sdk_ses::Config::builder()
            .region(Region::new("us-east-1"))
            .behavior_version(BehaviorVersion::latest())
            .credentials_provider(Credentials::new("test", "test", None, None, "unit-test"))
            .build();
        aws_sdk_ses::Client::from_conf(config)
    }

    #[test]
    fn ses_mail_sender_constructs_from_client() {
        let _sender = SesMailSender::new(client());
    }

    #[tokio::test]
    async fn ses_mail_sender_rejects_invalid_requests_before_aws_call() {
        let sender = SesMailSender::new(client());
        let missing_recipient = sender
            .send_mail(OutboundMailRequest {
                from_address: "contact@ahara.io".to_string(),
                to_addresses: Vec::new(),
                raw_message: b"From: contact@ahara.io\r\n\r\nbody".to_vec(),
            })
            .await;
        let header_injection = sender
            .send_mail(OutboundMailRequest {
                from_address: "contact@ahara.io\r\nBcc: attacker@example.com".to_string(),
                to_addresses: vec!["person@example.com".to_string()],
                raw_message: b"From: contact@ahara.io\r\n\r\nbody".to_vec(),
            })
            .await;

        assert!(matches!(missing_recipient, Err(AppError::Validation(_))));
        assert!(matches!(header_injection, Err(AppError::Validation(_))));
    }
}
