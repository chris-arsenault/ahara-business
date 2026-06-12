use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};
use shared::error::{AppError, AppResult};
use shared::inbound::limits::IngestLimits;
use shared::inbound::repository::{
    InboundRepository, PersistInboundMessageRequest, PersistRejectedInboundRequest,
};
use shared::inbound::routing::{
    InMemoryInboundRoutingLookup, InboundRoutingAddress, InboundRoutingDomain,
};
use shared::inbound::service::{InboundIngestService, IngestSummary};
use shared::inbound::types::{InboundMessageStatus, PersistedInboundMessage};
use shared::mail_security::SecurityDisposition;
use shared::ports::{RawMailMetadata, RawMailObject, RawMailStore};
use shared::routing::RoutingPolicy;
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
struct MemoryRawMailStore {
    objects: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    fail_keys: Arc<Mutex<BTreeSet<String>>>,
    metadata_count: Arc<Mutex<usize>>,
    get_count: Arc<Mutex<usize>>,
}

impl MemoryRawMailStore {
    fn insert(&self, key: &str, bytes: &[u8]) {
        self.objects
            .lock()
            .unwrap()
            .insert(key.to_string(), bytes.to_vec());
    }

    fn fail(&self, key: &str) {
        self.fail_keys.lock().unwrap().insert(key.to_string());
    }

    fn metadata_count(&self) -> usize {
        *self.metadata_count.lock().unwrap()
    }

    fn get_count(&self) -> usize {
        *self.get_count.lock().unwrap()
    }
}

#[async_trait]
impl RawMailStore for MemoryRawMailStore {
    async fn get_raw_mail_metadata(&self, key: &str) -> AppResult<RawMailMetadata> {
        *self.metadata_count.lock().unwrap() += 1;
        if self.fail_keys.lock().unwrap().contains(key) {
            return Err(AppError::ExternalService {
                service: "raw_mail_store",
                message: "temporary failure".to_string(),
            });
        }
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
        *self.get_count.lock().unwrap() += 1;
        if self.fail_keys.lock().unwrap().contains(key) {
            return Err(AppError::ExternalService {
                service: "raw_mail_store",
                message: "temporary failure".to_string(),
            });
        }
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
    seen: Arc<Mutex<BTreeMap<String, Uuid>>>,
    inbound: Arc<Mutex<Vec<PersistInboundMessageRequest>>>,
    rejected: Arc<Mutex<Vec<PersistRejectedInboundRequest>>>,
    recent_raw_bytes: Arc<Mutex<i64>>,
}

impl MemoryRepository {
    fn inbound(&self) -> Vec<PersistInboundMessageRequest> {
        self.inbound.lock().unwrap().clone()
    }

    fn rejected(&self) -> Vec<PersistRejectedInboundRequest> {
        self.rejected.lock().unwrap().clone()
    }

    fn set_recent_raw_bytes(&self, bytes: i64) {
        *self.recent_raw_bytes.lock().unwrap() = bytes;
    }
}

#[async_trait]
impl InboundRepository for MemoryRepository {
    async fn persist_inbound(
        &self,
        request: PersistInboundMessageRequest,
    ) -> AppResult<PersistedInboundMessage> {
        let mut seen = self.seen.lock().unwrap();
        let status = InboundMessageStatus::from_security_disposition(request.security.disposition);
        if let Some(id) = seen.get(&request.receipt.ses_message_id).copied() {
            return Ok(PersistedInboundMessage {
                id,
                status,
                security_disposition: request.security.disposition,
                idempotent: true,
            });
        }
        let id = Uuid::new_v4();
        seen.insert(request.receipt.ses_message_id.clone(), id);
        self.inbound.lock().unwrap().push(request);
        Ok(PersistedInboundMessage {
            id,
            status,
            security_disposition: self
                .inbound
                .lock()
                .unwrap()
                .last()
                .unwrap()
                .security
                .disposition,
            idempotent: false,
        })
    }

    async fn persist_rejected_inbound(
        &self,
        request: PersistRejectedInboundRequest,
    ) -> AppResult<PersistedInboundMessage> {
        let mut seen = self.seen.lock().unwrap();
        if let Some(id) = seen.get(&request.audit.ses_message_id).copied() {
            return Ok(PersistedInboundMessage {
                id,
                status: InboundMessageStatus::Rejected,
                security_disposition: SecurityDisposition::Rejected,
                idempotent: true,
            });
        }
        let id = Uuid::new_v4();
        seen.insert(request.audit.ses_message_id.clone(), id);
        self.rejected.lock().unwrap().push(request);
        Ok(PersistedInboundMessage {
            id,
            status: InboundMessageStatus::Rejected,
            security_disposition: SecurityDisposition::Rejected,
            idempotent: false,
        })
    }

    async fn recent_raw_mail_bytes(&self, _window_seconds: i64) -> AppResult<i64> {
        Ok(*self.recent_raw_bytes.lock().unwrap())
    }
}

fn service(
    raw: MemoryRawMailStore,
    repository: MemoryRepository,
    limits: IngestLimits,
) -> InboundIngestService {
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
        limits,
    )
}

fn event(id: &str, key: &str, recipient: &str, spam: &str, virus: &str) -> Value {
    json!({
        "Records": [{
            "eventSource": "aws:ses",
            "ses": {
                "mail": {
                    "messageId": id,
                    "timestamp": "2026-06-10T18:00:00.000Z",
                    "destination": [recipient]
                },
                "receipt": {
                    "recipients": [recipient],
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

fn multi_event(records: Vec<Value>) -> Value {
    json!({ "Records": records })
}

fn record(id: &str, key: &str, recipient: &str, spam: &str, virus: &str) -> Value {
    event(id, key, recipient, spam, virus)["Records"][0].clone()
}

#[tokio::test]
async fn inbound_service_accepts_clean_mail() {
    let raw = MemoryRawMailStore::default();
    raw.insert(
        "raw/clean",
        include_bytes!("fixtures/inbound/simple_text.eml"),
    );
    let repository = MemoryRepository::default();
    let service = service(raw, repository.clone(), IngestLimits::default());

    let summary = service
        .process_receipt_event(event(
            "ses-clean",
            "raw/clean",
            "contact@ahara.io",
            "PASS",
            "PASS",
        ))
        .await
        .unwrap();

    assert_eq!(summary.accepted, 1);
    assert_eq!(repository.inbound().len(), 1);
    assert_eq!(
        repository.inbound()[0].security.disposition,
        SecurityDisposition::Accepted
    );
}

#[tokio::test]
async fn inbound_service_quarantines_spam_and_rejects_virus_without_body_or_attachments() {
    let raw = MemoryRawMailStore::default();
    raw.insert(
        "raw/spam",
        include_bytes!("fixtures/inbound/simple_text.eml"),
    );
    raw.insert(
        "raw/virus",
        include_bytes!("fixtures/inbound/with_attachments.eml"),
    );
    let repository = MemoryRepository::default();
    let service = service(raw, repository.clone(), IngestLimits::default());

    let summary = service
        .process_receipt_event(multi_event(vec![
            record("ses-spam", "raw/spam", "contact@ahara.io", "FAIL", "PASS"),
            record("ses-virus", "raw/virus", "contact@ahara.io", "PASS", "FAIL"),
        ]))
        .await
        .unwrap();

    assert_eq!(summary.quarantined, 1);
    assert_eq!(summary.rejected, 1);
    assert_eq!(
        repository.inbound()[0].security.disposition,
        SecurityDisposition::Quarantined
    );
    assert_eq!(
        repository.rejected()[0].audit.from.address,
        "sender@example.test"
    );
    assert_eq!(
        repository.rejected()[0].audit.rejection_reason,
        "virus_failed"
    );
}

#[tokio::test]
async fn inbound_service_rejects_unknown_recipient_without_persisting_normal_mailbox() {
    let raw = MemoryRawMailStore::default();
    raw.insert(
        "raw/unknown",
        include_bytes!("fixtures/inbound/simple_text.eml"),
    );
    let repository = MemoryRepository::default();
    let service = service(raw, repository.clone(), IngestLimits::default());

    let summary = service
        .process_receipt_event(event(
            "ses-unknown",
            "raw/unknown",
            "unknown@ahara.io",
            "PASS",
            "PASS",
        ))
        .await
        .unwrap();

    assert_eq!(summary.rejected, 1);
    assert_eq!(repository.inbound().len(), 0);
    assert_eq!(
        repository.rejected()[0].audit.rejection_reason,
        "routing_allowlist_miss"
    );
}

#[tokio::test]
async fn inbound_service_treats_cap_rejection_as_non_transient() {
    let raw = MemoryRawMailStore::default();
    raw.insert(
        "raw/large",
        include_bytes!("fixtures/inbound/simple_text.eml"),
    );
    let repository = MemoryRepository::default();
    let service = service(raw, repository.clone(), IngestLimits::new(4, 20, 25));

    let summary = service
        .process_receipt_event(event(
            "ses-large",
            "raw/large",
            "contact@ahara.io",
            "PASS",
            "PASS",
        ))
        .await
        .unwrap();

    assert_eq!(summary.rejected, 1);
    assert_eq!(
        repository.rejected()[0].audit.rejection_reason,
        "limit_exceeded_raw_mime_bytes"
    );
}

#[tokio::test]
async fn inbound_service_rejects_oversize_raw_object_before_fetching_body() {
    let raw = MemoryRawMailStore::default();
    raw.insert("raw/oversize", b"oversize raw object");
    let repository = MemoryRepository::default();
    let limits = IngestLimits::default().with_raw_mail_controls(4, 100, 3600);
    let service = service(raw.clone(), repository.clone(), limits);

    let summary = service
        .process_receipt_event(event(
            "ses-oversize",
            "raw/oversize",
            "contact@ahara.io",
            "PASS",
            "PASS",
        ))
        .await
        .unwrap();

    assert_eq!(summary.rejected, 1);
    assert_eq!(summary.oversize_rejected, 1);
    assert_eq!(summary.hourly_bytes_rejected, 0);
    assert_eq!(repository.inbound().len(), 0);
    assert_eq!(raw.metadata_count(), 1);
    assert_eq!(raw.get_count(), 0);
    assert_eq!(
        repository.rejected()[0].audit.rejection_reason,
        "limit_exceeded_raw_mail_object_bytes"
    );
    assert_eq!(
        repository.rejected()[0].audit.size_bytes,
        Some(b"oversize raw object".len() as i64)
    );
}

#[tokio::test]
async fn inbound_service_rejects_when_recent_raw_byte_window_would_exceed_limit() {
    let raw = MemoryRawMailStore::default();
    raw.insert("raw/hourly", b"hour");
    let repository = MemoryRepository::default();
    repository.set_recent_raw_bytes(9);
    let limits = IngestLimits::default().with_raw_mail_controls(100, 10, 3600);
    let service = service(raw.clone(), repository.clone(), limits);

    let summary = service
        .process_receipt_event(event(
            "ses-hourly",
            "raw/hourly",
            "contact@ahara.io",
            "PASS",
            "PASS",
        ))
        .await
        .unwrap();

    assert_eq!(summary.rejected, 1);
    assert_eq!(summary.oversize_rejected, 0);
    assert_eq!(summary.hourly_bytes_rejected, 1);
    assert_eq!(repository.inbound().len(), 0);
    assert_eq!(raw.metadata_count(), 1);
    assert_eq!(raw.get_count(), 0);
    assert_eq!(
        repository.rejected()[0].audit.rejection_reason,
        "limit_exceeded_recent_raw_mail_bytes"
    );
    assert_eq!(repository.rejected()[0].audit.size_bytes, Some(4));
}

#[tokio::test]
async fn inbound_service_counts_transient_raw_store_failures() {
    let raw = MemoryRawMailStore::default();
    raw.fail("raw/fail");
    let repository = MemoryRepository::default();
    let service = service(raw, repository, IngestLimits::default());

    let summary = service
        .process_receipt_event(event(
            "ses-fail",
            "raw/fail",
            "contact@ahara.io",
            "PASS",
            "PASS",
        ))
        .await
        .unwrap();

    assert_eq!(
        summary,
        IngestSummary {
            processed: 1,
            accepted: 0,
            quarantined: 0,
            rejected: 0,
            oversize_rejected: 0,
            hourly_bytes_rejected: 0,
            idempotent: 0,
            failed: 1,
        }
    );
}

#[tokio::test]
async fn inbound_service_counts_retry_idempotency() {
    let raw = MemoryRawMailStore::default();
    raw.insert(
        "raw/retry",
        include_bytes!("fixtures/inbound/simple_text.eml"),
    );
    let repository = MemoryRepository::default();
    let service = service(raw, repository.clone(), IngestLimits::default());

    let first = service
        .process_receipt_event(event(
            "ses-retry",
            "raw/retry",
            "contact@ahara.io",
            "PASS",
            "PASS",
        ))
        .await
        .unwrap();
    let second = service
        .process_receipt_event(event(
            "ses-retry",
            "raw/retry",
            "contact@ahara.io",
            "PASS",
            "PASS",
        ))
        .await
        .unwrap();

    assert_eq!(first.accepted, 1);
    assert_eq!(second.accepted, 1);
    assert_eq!(second.idempotent, 1);
    assert_eq!(repository.inbound().len(), 1);
}
