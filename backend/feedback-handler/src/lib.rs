use serde_json::{Value, json};
use shared::config::AppConfig;
use shared::db::connect_pool;
use shared::error::AppResult;
use shared::feedback::{FeedbackService, PgFeedbackService};
#[cfg(test)]
use shared::observability::mail_metric_payload;
use shared::observability::{CountMetric, emit_mail_metric};

pub async fn handle_event(
    payload: Value,
    request_id: &str,
    config: &AppConfig,
) -> AppResult<Value> {
    let pool = connect_pool(config).await?;
    let service = PgFeedbackService::new(pool);
    handle_event_with_service(payload, request_id, config, &service).await
}

pub async fn handle_event_with_service(
    payload: Value,
    request_id: &str,
    config: &AppConfig,
    service: &dyn FeedbackService,
) -> AppResult<Value> {
    let summary = service.process_feedback(payload).await?;
    let metrics = feedback_operational_metrics(&summary)?;
    emit_mail_metric("feedback-handler", &config.mail.domain, &metrics)?;
    tracing::info!(
        request_id = %request_id,
        service = shared::service_name(),
        mail_domain = %config.mail.domain,
        processed = summary.processed,
        suppressed_recipients = summary.suppressed_recipients,
        bounced = summary.bounced,
        complained = summary.complained,
        "feedback handler completed"
    );
    Ok(json!({
        "status": "ok",
        "handler": "feedback-handler",
        "summary": summary,
    }))
}

fn feedback_operational_metrics(
    summary: &shared::feedback::FeedbackProcessSummary,
) -> AppResult<Vec<CountMetric>> {
    Ok(vec![
        CountMetric::new("FeedbackProcessed", summary.processed as u64)?,
        CountMetric::new("FeedbackBounced", summary.bounced as u64)?,
        CountMetric::new("FeedbackComplained", summary.complained as u64)?,
        CountMetric::new(
            "FeedbackSuppressedRecipients",
            summary.suppressed_recipients as u64,
        )?,
    ])
}

#[cfg(test)]
fn feedback_operational_metric_payload(
    mail_domain: &str,
    summary: &shared::feedback::FeedbackProcessSummary,
) -> AppResult<Value> {
    mail_metric_payload(
        "feedback-handler",
        mail_domain,
        &feedback_operational_metrics(summary)?,
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use shared::config::{
        ApiConfig, AppConfig, CognitoConfig, DatabaseConfig, FeedbackConfig, MailConfig,
    };
    use shared::feedback::InMemoryFeedbackService;

    use super::{feedback_operational_metric_payload, handle_event_with_service};

    fn config() -> AppConfig {
        AppConfig {
            database: DatabaseConfig {
                host: "localhost".to_string(),
                port: 5432,
                name: "ahara_business".to_string(),
                username: "app".to_string(),
                password: "password".to_string(),
            },
            mail: MailConfig {
                domain: "ahara.io".to_string(),
                raw_mail_bucket: "ahara-business-raw-mail-test".to_string(),
                raw_mail_prefix: "raw/".to_string(),
            },
            feedback: FeedbackConfig {
                bounce_topic_arn: "arn:aws:sns:::bounces".to_string(),
                complaint_topic_arn: "arn:aws:sns:::complaints".to_string(),
            },
            api: ApiConfig {
                api_base_url: "https://api.example.test".to_string(),
                app_base_url: "https://app.example.test".to_string(),
            },
            cognito: CognitoConfig {
                user_pool_id: "us-east-1_pool".to_string(),
                client_id: "client-123".to_string(),
                domain: "auth.example.test".to_string(),
                issuer: "https://issuer.example.test".to_string(),
            },
        }
    }

    #[tokio::test]
    async fn handler_processes_feedback_event() {
        let service = InMemoryFeedbackService::default();
        let response = handle_event_with_service(
            json!({
                "Records": [{
                    "Sns": {
                        "Message": json!({
                            "notificationType": "Complaint",
                            "mail": { "messageId": "ses-message-1" },
                            "complaint": {
                                "complainedRecipients": [
                                    { "emailAddress": "person@example.com" }
                                ]
                            }
                        }).to_string()
                    }
                }]
            }),
            "request-1",
            &config(),
            &service,
        )
        .await
        .unwrap();

        assert_eq!(response["status"], "ok");
        assert_eq!(response["handler"], "feedback-handler");
        assert_eq!(response["summary"]["processed"], 1);
        assert_eq!(response["summary"]["suppressed_recipients"], 1);
        assert_eq!(response["summary"]["complained"], 1);
    }

    #[tokio::test]
    async fn operational_metric_payload_is_count_only() {
        let service = InMemoryFeedbackService::default();
        let response = handle_event_with_service(
            json!({
                "Records": [{
                    "Sns": {
                        "Message": json!({
                            "notificationType": "Complaint",
                            "mail": { "messageId": "ses-message-1" },
                            "complaint": {
                                "complainedRecipients": [
                                    { "emailAddress": "person@example.com" }
                                ]
                            }
                        }).to_string()
                    }
                }]
            }),
            "request-1",
            &config(),
            &service,
        )
        .await
        .unwrap();
        let metrics = feedback_operational_metric_payload(
            "ahara.io",
            &serde_json::from_value(response["summary"].clone()).unwrap(),
        )
        .unwrap();
        let serialized = serde_json::to_string(&metrics).unwrap();

        assert!(serialized.contains("FeedbackComplained"));
        assert!(!serialized.contains("person@example.com"));
        assert!(!serialized.contains("ses-message-1"));
    }
}
