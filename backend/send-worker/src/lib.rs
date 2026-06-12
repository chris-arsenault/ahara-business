use std::sync::Arc;

use serde_json::{Value, json};
use shared::config::AppConfig;
use shared::db::connect_pool;
use shared::error::AppResult;
#[cfg(test)]
use shared::observability::mail_metric_payload;
use shared::observability::{CountMetric, emit_mail_metric};
use shared::outbound::{OutboundSendWorker, PgOutboundService};
use shared::raw_mail_store::S3RawMailStore;
use shared::retention::{
    PgRawMailRetentionService, RawMailRetentionService, RawMailRetentionSummary,
};
use shared::ses_mail_sender::SesMailSender;

pub async fn handle_event(
    payload: Value,
    request_id: &str,
    config: &AppConfig,
) -> AppResult<Value> {
    let pool = connect_pool(config).await?;
    let outbound = Arc::new(PgOutboundService::new(
        pool.clone(),
        config.mail.domain.clone(),
    ));
    let mail_sender = Arc::new(SesMailSender::from_env().await);
    let worker = OutboundSendWorker::new(outbound, mail_sender, request_id);
    let raw_mail_store = Arc::new(S3RawMailStore::from_env(&config.mail).await);
    let retention = PgRawMailRetentionService::new(pool, raw_mail_store);
    handle_event_with_worker_and_retention(payload, request_id, config, &worker, &retention).await
}

pub async fn handle_event_with_worker(
    _payload: Value,
    request_id: &str,
    config: &AppConfig,
    worker: &OutboundSendWorker,
) -> AppResult<Value> {
    handle_event_with_worker_and_retention(
        _payload,
        request_id,
        config,
        worker,
        &NoopRawMailRetentionService,
    )
    .await
}

pub async fn handle_event_with_worker_and_retention(
    _payload: Value,
    request_id: &str,
    config: &AppConfig,
    worker: &OutboundSendWorker,
    retention: &dyn RawMailRetentionService,
) -> AppResult<Value> {
    let summary = worker.run_once().await?;
    let retention_summary = retention.cleanup_due_raw_mail().await?;
    let metrics = send_worker_operational_metrics(&summary)?;
    emit_mail_metric("send-worker", &config.mail.domain, &metrics)?;
    let retention_metrics = retention_operational_metrics(&retention_summary)?;
    emit_mail_metric("retention-cleanup", &config.mail.domain, &retention_metrics)?;
    tracing::info!(
        request_id = %request_id,
        service = shared::service_name(),
        mail_domain = %config.mail.domain,
        claimed = summary.claimed,
        sent = summary.sent,
        retried = summary.retried,
        failed = summary.failed,
        suppressed = summary.suppressed,
        raw_retention_candidates = retention_summary.candidates,
        raw_retention_deleted = retention_summary.deleted,
        raw_retention_failed = retention_summary.failed,
        "send worker completed"
    );
    Ok(json!({
        "status": "ok",
        "handler": "send-worker",
        "summary": summary,
        "retention": retention_summary,
    }))
}

fn send_worker_operational_metrics(
    summary: &shared::outbound::OutboundSendSummary,
) -> AppResult<Vec<CountMetric>> {
    Ok(vec![
        CountMetric::new("OutboundClaimed", summary.claimed as u64)?,
        CountMetric::new("OutboundSent", summary.sent as u64)?,
        CountMetric::new("OutboundRetried", summary.retried as u64)?,
        CountMetric::new("OutboundFailed", summary.failed as u64)?,
        CountMetric::new("OutboundSuppressed", summary.suppressed as u64)?,
    ])
}

fn retention_operational_metrics(summary: &RawMailRetentionSummary) -> AppResult<Vec<CountMetric>> {
    Ok(vec![
        CountMetric::new("RawMailRetentionCandidates", summary.candidates as u64)?,
        CountMetric::new("RawMailRetentionDeleted", summary.deleted as u64)?,
        CountMetric::new("RawMailRetentionFailed", summary.failed as u64)?,
    ])
}

struct NoopRawMailRetentionService;

#[async_trait::async_trait]
impl RawMailRetentionService for NoopRawMailRetentionService {
    async fn cleanup_due_raw_mail(&self) -> AppResult<RawMailRetentionSummary> {
        Ok(RawMailRetentionSummary::default())
    }
}

#[cfg(test)]
fn send_worker_operational_metric_payload(
    mail_domain: &str,
    summary: &shared::outbound::OutboundSendSummary,
) -> AppResult<Value> {
    mail_metric_payload(
        "send-worker",
        mail_domain,
        &send_worker_operational_metrics(summary)?,
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;

    use async_trait::async_trait;
    use serde_json::json;
    use shared::config::{
        ApiConfig, AppConfig, CognitoConfig, DatabaseConfig, FeedbackConfig, MailConfig,
    };
    use shared::error::AppResult;
    use shared::outbound::{
        ComposeMessageRequest, InMemoryOutboundService, OutboundSendWorker, OutboundService,
    };
    use shared::ports::{MailSender, OutboundMailRequest, OutboundMailResponse};

    use super::{handle_event_with_worker, send_worker_operational_metric_payload};

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
    async fn handler_runs_send_worker_once() {
        let outbound = InMemoryOutboundService::new("ahara.io");
        outbound
            .compose_message(ComposeMessageRequest {
                from_address: "contact@ahara.io".to_string(),
                to: vec!["person@example.com".to_string()],
                cc: Vec::new(),
                bcc: Vec::new(),
                subject: "Send".to_string(),
                body_text: "body".to_string(),
                attachments: Vec::new(),
            })
            .await
            .unwrap();
        let mail_sender = RecordingMailSender::default();
        let worker =
            OutboundSendWorker::new(Arc::new(outbound), Arc::new(mail_sender), "request-1");

        let response = handle_event_with_worker(
            json!({ "kind": "scheduled" }),
            "request-1",
            &config(),
            &worker,
        )
        .await
        .unwrap();

        assert_eq!(response["status"], "ok");
        assert_eq!(response["handler"], "send-worker");
        assert_eq!(response["summary"]["claimed"], 1);
        assert_eq!(response["summary"]["sent"], 1);
    }

    #[tokio::test]
    async fn operational_metric_payload_is_count_only() {
        let outbound = InMemoryOutboundService::new("ahara.io");
        outbound
            .compose_message(ComposeMessageRequest {
                from_address: "contact@ahara.io".to_string(),
                to: vec!["person@example.com".to_string()],
                cc: Vec::new(),
                bcc: Vec::new(),
                subject: "Sensitive outbound subject".to_string(),
                body_text: "Sensitive outbound body".to_string(),
                attachments: Vec::new(),
            })
            .await
            .unwrap();
        let worker = OutboundSendWorker::new(
            Arc::new(outbound),
            Arc::new(RecordingMailSender::default()),
            "request-1",
        );
        let response = handle_event_with_worker(
            json!({ "kind": "scheduled" }),
            "request-1",
            &config(),
            &worker,
        )
        .await
        .unwrap();
        let metrics = send_worker_operational_metric_payload(
            "ahara.io",
            &serde_json::from_value(response["summary"].clone()).unwrap(),
        )
        .unwrap();
        let serialized = serde_json::to_string(&metrics).unwrap();

        assert!(serialized.contains("OutboundSent"));
        assert!(!serialized.contains("Sensitive outbound subject"));
        assert!(!serialized.contains("Sensitive outbound body"));
        assert!(!serialized.contains("contact@ahara.io"));
        assert!(!serialized.contains("person@example.com"));
    }

    #[derive(Debug, Default)]
    struct RecordingMailSender {
        sent: Mutex<Vec<OutboundMailRequest>>,
    }

    #[async_trait]
    impl MailSender for RecordingMailSender {
        async fn send_mail(&self, request: OutboundMailRequest) -> AppResult<OutboundMailResponse> {
            self.sent.lock().unwrap().push(request);
            Ok(OutboundMailResponse {
                provider_message_id: "test-message-1".to_string(),
            })
        }
    }
}
