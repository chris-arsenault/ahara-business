use std::sync::Arc;

use serde_json::{Value, json};
use shared::config::AppConfig;
use shared::db::connect_pool;
use shared::error::AppResult;
use shared::forwarding::{ForwardingPlanner, PgForwardingRuleService};
use shared::inbound::limits::IngestLimits;
use shared::inbound::repository::PgInboundRepository;
use shared::inbound::routing::PgInboundRoutingLookup;
use shared::inbound::service::InboundIngestService;
#[cfg(test)]
use shared::observability::mail_metric_payload;
use shared::observability::{CountMetric, emit_mail_metric};
use shared::outbound::PgOutboundService;
use shared::raw_mail_store::S3RawMailStore;

pub async fn handle_event(
    payload: Value,
    request_id: &str,
    config: &AppConfig,
) -> AppResult<Value> {
    let service = build_service(config).await?;
    handle_event_with_service(payload, request_id, &config.mail.domain, &service).await
}

pub async fn handle_event_with_service(
    payload: Value,
    request_id: &str,
    mail_domain: &str,
    service: &InboundIngestService,
) -> AppResult<Value> {
    let summary = service.process_receipt_event(payload).await?;
    let metrics = ingest_operational_metrics(&summary)?;
    emit_mail_metric("ingest", mail_domain, &metrics)?;
    tracing::info!(
        request_id = %request_id,
        service = shared::service_name(),
        mail_domain = %mail_domain,
        processed = summary.processed,
        accepted = summary.accepted,
        quarantined = summary.quarantined,
        rejected = summary.rejected,
        oversize_rejected = summary.oversize_rejected,
        hourly_bytes_rejected = summary.hourly_bytes_rejected,
        failed = summary.failed,
        "inbound ingest completed"
    );
    Ok(json!({
        "status": "ok",
        "handler": "ingest",
        "summary": summary,
    }))
}

fn ingest_operational_metrics(
    summary: &shared::inbound::service::IngestSummary,
) -> AppResult<Vec<CountMetric>> {
    Ok(vec![
        CountMetric::new("InboundProcessed", summary.processed as u64)?,
        CountMetric::new("InboundAccepted", summary.accepted as u64)?,
        CountMetric::new("InboundQuarantined", summary.quarantined as u64)?,
        CountMetric::new("InboundRejected", summary.rejected as u64)?,
        CountMetric::new("InboundFailed", summary.failed as u64)?,
        CountMetric::new("InboundOversizeRejected", summary.oversize_rejected as u64)?,
        CountMetric::new(
            "InboundHourlyBytesRejected",
            summary.hourly_bytes_rejected as u64,
        )?,
    ])
}

#[cfg(test)]
fn ingest_operational_metric_payload(
    mail_domain: &str,
    summary: &shared::inbound::service::IngestSummary,
) -> AppResult<Value> {
    mail_metric_payload("ingest", mail_domain, &ingest_operational_metrics(summary)?)
}

pub async fn build_service(config: &AppConfig) -> AppResult<InboundIngestService> {
    let pool = connect_pool(config).await?;
    let raw_mail_store = S3RawMailStore::from_env(&config.mail).await;
    let forwarding_planner = Arc::new(ForwardingPlanner::new(
        Arc::new(PgForwardingRuleService::new(pool.clone())),
        Arc::new(PgOutboundService::new(
            pool.clone(),
            config.mail.domain.clone(),
        )),
    ));
    Ok(InboundIngestService::new(
        Arc::new(raw_mail_store),
        Arc::new(PgInboundRoutingLookup::new(pool.clone())),
        Arc::new(PgInboundRepository::new(pool)),
        IngestLimits::default(),
    )
    .with_raw_mail_location(
        config.mail.raw_mail_bucket.clone(),
        config.mail.raw_mail_prefix.clone(),
    )
    .with_forwarding_planner(forwarding_planner))
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use serde_json::json;
    use shared::error::{AppError, AppResult};
    use shared::inbound::limits::IngestLimits;
    use shared::inbound::repository::{
        InboundRepository, PersistInboundMessageRequest, PersistRejectedInboundRequest,
    };
    use shared::inbound::routing::{
        InMemoryInboundRoutingLookup, InboundRoutingAddress, InboundRoutingDomain,
    };
    use shared::inbound::service::InboundIngestService;
    use shared::inbound::types::{InboundMessageStatus, PersistedInboundMessage};
    use shared::mail_security::SecurityDisposition;
    use shared::ports::{RawMailMetadata, RawMailObject, RawMailStore};
    use shared::routing::RoutingPolicy;
    use uuid::Uuid;

    use super::{handle_event_with_service, ingest_operational_metric_payload};

    #[tokio::test]
    async fn handler_rejects_invalid_ses_event() {
        let err = handle_event_with_service(
            json!({ "kind": "dummy" }),
            "request-1",
            "ahara.io",
            &service(MemoryRawMailStore::default(), MemoryRepository::default()),
        )
        .await
        .unwrap_err();

        assert!(err.public_message().contains("SES receipt event"));
    }

    #[tokio::test]
    async fn handler_persists_clean_ses_receipt() {
        let raw = MemoryRawMailStore::default();
        raw.insert(
            "raw/clean",
            include_bytes!("../../shared/tests/fixtures/inbound/simple_text.eml"),
        );
        let repository = MemoryRepository::default();
        let response = handle_event_with_service(
            event("ses-clean", "raw/clean", "PASS", "PASS"),
            "request-1",
            "ahara.io",
            &service(raw, repository.clone()),
        )
        .await
        .unwrap();

        assert_eq!(response["status"], "ok");
        assert_eq!(response["handler"], "ingest");
        assert_eq!(response["summary"]["accepted"], 1);
        assert_eq!(repository.inbound_count(), 1);
        let body = serde_json::to_string(&response).unwrap();
        assert!(!body.contains("Hello from plain text"));
        assert!(!body.contains("From:"));
    }

    #[tokio::test]
    async fn handler_persists_lambda_invoked_ses_receipt_from_configured_raw_location() {
        let raw = MemoryRawMailStore::default();
        raw.insert(
            "raw/ses-lambda",
            include_bytes!("../../shared/tests/fixtures/inbound/simple_text.eml"),
        );
        let repository = MemoryRepository::default();
        let response = handle_event_with_service(
            lambda_invoked_event("ses-lambda", "PASS", "PASS"),
            "request-1",
            "ahara.io",
            &service(raw, repository.clone())
                .with_raw_mail_location("ahara-business-raw-mail-test", "raw/"),
        )
        .await
        .unwrap();

        assert_eq!(response["status"], "ok");
        assert_eq!(response["summary"]["accepted"], 1);
        assert_eq!(repository.inbound_count(), 1);
    }

    #[tokio::test]
    async fn handler_reports_rejected_virus_receipt_without_body_leakage() {
        let raw = MemoryRawMailStore::default();
        raw.insert(
            "raw/virus",
            include_bytes!("../../shared/tests/fixtures/inbound/with_attachments.eml"),
        );
        let repository = MemoryRepository::default();
        let response = handle_event_with_service(
            event("ses-virus", "raw/virus", "PASS", "FAIL"),
            "request-1",
            "ahara.io",
            &service(raw, repository.clone()),
        )
        .await
        .unwrap();

        assert_eq!(response["summary"]["rejected"], 1);
        assert_eq!(repository.rejected_count(), 1);
        let body = serde_json::to_string(&response).unwrap();
        assert!(!body.contains("Attached metadata only"));
        assert!(!body.contains("invoice.pdf"));
    }

    #[tokio::test]
    async fn operational_metric_payload_is_count_only() {
        let raw = MemoryRawMailStore::default();
        raw.insert(
            "raw/clean",
            include_bytes!("../../shared/tests/fixtures/inbound/simple_text.eml"),
        );
        let repository = MemoryRepository::default();
        let response = handle_event_with_service(
            event("ses-clean", "raw/clean", "PASS", "PASS"),
            "request-1",
            "ahara.io",
            &service(raw, repository),
        )
        .await
        .unwrap();
        let metrics = ingest_operational_metric_payload(
            "ahara.io",
            &serde_json::from_value(response["summary"].clone()).unwrap(),
        )
        .unwrap();
        let serialized = serde_json::to_string(&metrics).unwrap();

        assert!(serialized.contains("InboundProcessed"));
        assert!(serialized.contains("InboundOversizeRejected"));
        assert!(!serialized.contains("Hello from plain text"));
        assert!(!serialized.contains("sender@example.test"));
        assert!(!serialized.contains("contact@ahara.io"));
        assert!(!serialized.contains("raw/clean"));
        assert!(!serialized.contains("invoice.pdf"));
    }

    #[derive(Debug, Clone, Default)]
    struct MemoryRawMailStore {
        objects: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    }

    impl MemoryRawMailStore {
        fn insert(&self, key: &str, bytes: &[u8]) {
            self.objects
                .lock()
                .unwrap()
                .insert(key.to_string(), bytes.to_vec());
        }
    }

    #[async_trait]
    impl RawMailStore for MemoryRawMailStore {
        async fn get_raw_mail_metadata(&self, key: &str) -> AppResult<RawMailMetadata> {
            self.objects
                .lock()
                .unwrap()
                .get(key)
                .map(|bytes| RawMailMetadata {
                    key: key.to_string(),
                    size_bytes: bytes.len(),
                })
                .ok_or_else(|| AppError::NotFound(format!("raw {key}")))
        }

        async fn get_raw_mail(&self, key: &str) -> AppResult<RawMailObject> {
            self.objects
                .lock()
                .unwrap()
                .get(key)
                .cloned()
                .map(|bytes| RawMailObject {
                    key: key.to_string(),
                    bytes,
                })
                .ok_or_else(|| AppError::NotFound(format!("raw {key}")))
        }

        async fn put_raw_mail(&self, object: RawMailObject) -> AppResult<()> {
            self.insert(&object.key, &object.bytes);
            Ok(())
        }
    }

    #[derive(Debug, Clone, Default)]
    struct MemoryRepository {
        seen: Arc<Mutex<BTreeSet<String>>>,
        inbound: Arc<Mutex<Vec<PersistInboundMessageRequest>>>,
        rejected: Arc<Mutex<Vec<PersistRejectedInboundRequest>>>,
    }

    impl MemoryRepository {
        fn inbound_count(&self) -> usize {
            self.inbound.lock().unwrap().len()
        }

        fn rejected_count(&self) -> usize {
            self.rejected.lock().unwrap().len()
        }
    }

    #[async_trait]
    impl InboundRepository for MemoryRepository {
        async fn persist_inbound(
            &self,
            request: PersistInboundMessageRequest,
        ) -> AppResult<PersistedInboundMessage> {
            let mut seen = self.seen.lock().unwrap();
            let idempotent = !seen.insert(request.receipt.ses_message_id.clone());
            let status =
                InboundMessageStatus::from_security_disposition(request.security.disposition);
            let disposition = request.security.disposition;
            if !idempotent {
                self.inbound.lock().unwrap().push(request);
            }
            Ok(PersistedInboundMessage {
                id: Uuid::new_v4(),
                status,
                security_disposition: disposition,
                idempotent,
            })
        }

        async fn persist_rejected_inbound(
            &self,
            request: PersistRejectedInboundRequest,
        ) -> AppResult<PersistedInboundMessage> {
            let mut seen = self.seen.lock().unwrap();
            let idempotent = !seen.insert(request.audit.ses_message_id.clone());
            if !idempotent {
                self.rejected.lock().unwrap().push(request);
            }
            Ok(PersistedInboundMessage {
                id: Uuid::new_v4(),
                status: InboundMessageStatus::Rejected,
                security_disposition: SecurityDisposition::Rejected,
                idempotent,
            })
        }

        async fn recent_raw_mail_bytes(&self, _window_seconds: i64) -> AppResult<i64> {
            Ok(0)
        }
    }

    fn service(raw: MemoryRawMailStore, repository: MemoryRepository) -> InboundIngestService {
        InboundIngestService::new(
            Arc::new(raw),
            Arc::new(InMemoryInboundRoutingLookup::with_domains([
                InboundRoutingDomain {
                    id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
                    domain_name: "ahara.io".to_string(),
                    routing_policy: RoutingPolicy::Allowlist,
                    active: true,
                    addresses: vec![InboundRoutingAddress {
                        id: Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
                        local_part: "contact".to_string(),
                        active: true,
                    }],
                },
            ])),
            Arc::new(repository),
            IngestLimits::default(),
        )
    }

    fn event(id: &str, key: &str, spam: &str, virus: &str) -> serde_json::Value {
        json!({
            "Records": [{
                "eventSource": "aws:ses",
                "ses": {
                    "mail": {
                        "messageId": id,
                        "timestamp": "2026-06-10T18:00:00.000Z",
                        "destination": ["contact@ahara.io"]
                    },
                    "receipt": {
                        "recipients": ["contact@ahara.io"],
                        "spfVerdict": { "status": "PASS" },
                        "dkimVerdict": { "status": "PASS" },
                        "dmarcVerdict": { "status": "PASS" },
                        "spamVerdict": { "status": spam },
                        "virusVerdict": { "status": virus },
                        "action": {
                            "type": "S3",
                            "bucketName": "ahara-business-raw-mail-test",
                            "objectKey": key
                        }
                    }
                }
            }]
        })
    }

    fn lambda_invoked_event(id: &str, spam: &str, virus: &str) -> serde_json::Value {
        json!({
            "Records": [{
                "eventSource": "aws:ses",
                "ses": {
                    "mail": {
                        "messageId": id,
                        "timestamp": "2026-06-10T18:00:00.000Z",
                        "destination": ["contact@ahara.io"]
                    },
                    "receipt": {
                        "recipients": ["contact@ahara.io"],
                        "spfVerdict": { "status": "PASS" },
                        "dkimVerdict": { "status": "PASS" },
                        "dmarcVerdict": { "status": "PASS" },
                        "spamVerdict": { "status": spam },
                        "virusVerdict": { "status": virus },
                        "action": {
                            "type": "Lambda",
                            "functionArn": "arn:aws:lambda:us-east-1:123456789012:function:ahara-business-ingest",
                            "invocationType": "Event"
                        }
                    }
                }
            }]
        })
    }
}
