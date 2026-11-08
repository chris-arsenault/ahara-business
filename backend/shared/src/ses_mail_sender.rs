use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_sesv2::Client;
use aws_sdk_sesv2::primitives::Blob;
use aws_sdk_sesv2::types::{Destination, EmailContent, RawMessage};

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
        if request.raw_message.len() > 40 * 1024 * 1024 {
            return Err(AppError::Validation(
                "encoded message exceeds the SES 40 MB limit".into(),
            ));
        }
        let raw_message = Self::raw_message(request.raw_message)?;
        let output = self
            .client
            .send_email()
            .from_email_address(request.from_address)
            .destination(
                Destination::builder()
                    .set_to_addresses(Some(request.to_addresses))
                    .build(),
            )
            .content(EmailContent::builder().raw(raw_message).build())
            .send()
            .await
            .map_err(|err| AppError::ExternalService {
                service: "ses",
                message: err.to_string(),
            })?;

        Ok(OutboundMailResponse {
            provider_message_id: output
                .message_id()
                .ok_or_else(|| AppError::Internal("SES returned no message id".into()))?
                .to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use aws_config::Region;
    use aws_sdk_sesv2::config::{BehaviorVersion, Credentials};

    use crate::error::AppError;
    use crate::ports::{MailSender, OutboundMailRequest};

    use super::SesMailSender;

    fn client() -> aws_sdk_sesv2::Client {
        let config = aws_sdk_sesv2::Config::builder()
            .region(Region::new("us-east-1"))
            .behavior_version(BehaviorVersion::latest())
            .credentials_provider(Credentials::new("test", "test", None, None, "unit-test"))
            .build();
        aws_sdk_sesv2::Client::from_conf(config)
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
