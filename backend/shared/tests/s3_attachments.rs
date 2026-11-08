use std::{
    collections::BTreeMap,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use base64::Engine;
use sha2::{Digest, Sha256};
use shared::attachment_storage::{AttachmentStorage, SignedAttachmentUrl, StoredAttachment};
use shared::attachment_uploads::{
    AttachmentUploadService, CreateAttachmentUpload, PgAttachmentUploadService,
    UploadedAttachmentRef,
};
use shared::db::{DbPool, MAIL_MODEL_MIGRATION};
use shared::error::{AppError, AppResult};
use shared::outbound::{
    ComposeMessageRequest, MAX_OUTBOUND_ATTACHMENT_BYTES, OutboundAttachmentInput, OutboundService,
    PgOutboundService,
};

struct Postgres(String);
impl Drop for Postgres {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.0])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

async fn database() -> (Postgres, DbPool) {
    let name = format!("ahara-business-s3-test-{}", uuid::Uuid::new_v4());
    let run = Command::new("docker")
        .args([
            "run",
            "-d",
            "--rm",
            "--name",
            &name,
            "-p",
            "127.0.0.1::5432",
            "-e",
            "POSTGRES_PASSWORD=postgres",
            "postgres:16-alpine",
        ])
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let container = Postgres(name);
    let port = Command::new("docker")
        .args(["port", &container.0, "5432/tcp"])
        .output()
        .unwrap();
    let address = String::from_utf8(port.stdout).unwrap();
    let url = format!("postgres://postgres:postgres@{}/postgres", address.trim());
    for _ in 0..60 {
        if let Ok(pool) = sqlx::postgres::PgPoolOptions::new()
            .max_connections(3)
            .connect(&url)
            .await
        {
            sqlx::raw_sql(MAIL_MODEL_MIGRATION)
                .execute(&pool)
                .await
                .unwrap();
            return (container, pool);
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    panic!("test PostgreSQL did not start");
}

#[derive(Default)]
struct Storage {
    stage: Mutex<BTreeMap<String, Vec<u8>>>,
    final_objects: Mutex<BTreeMap<String, Vec<u8>>>,
}
fn signed() -> SignedAttachmentUrl {
    SignedAttachmentUrl {
        url: "https://mail.s3.amazonaws.com/signed".into(),
        headers: Default::default(),
        expires_in_seconds: 900,
    }
}
fn request(bytes: &[u8]) -> CreateAttachmentUpload {
    CreateAttachmentUpload {
        filename: "notes.txt".into(),
        content_type: "text/plain".into(),
        size_bytes: bytes.len() as i64,
        checksum_sha256: base64::engine::general_purpose::STANDARD.encode(Sha256::digest(bytes)),
    }
}

#[async_trait]
impl AttachmentStorage for Storage {
    async fn grant_upload(
        &self,
        _: &str,
        _: &CreateAttachmentUpload,
    ) -> AppResult<SignedAttachmentUrl> {
        Ok(signed())
    }
    async fn finalize_upload(
        &self,
        key: &str,
        expected: &CreateAttachmentUpload,
    ) -> AppResult<StoredAttachment> {
        let stage = self.stage.lock().unwrap();
        let bytes = stage
            .get(key)
            .ok_or_else(|| AppError::Validation("missing object".into()))?;
        if request(bytes).checksum_sha256 != expected.checksum_sha256
            || bytes.len() as i64 != expected.size_bytes
        {
            return Err(AppError::Validation("mismatched object".into()));
        }
        let key = format!("attachments/files/{}", uuid::Uuid::new_v4());
        self.final_objects
            .lock()
            .unwrap()
            .insert(key.clone(), bytes.clone());
        Ok(StoredAttachment {
            key,
            version_id: "immutable-version".into(),
            size_bytes: bytes.len() as i64,
        })
    }
    async fn read(&self, object: &StoredAttachment) -> AppResult<Vec<u8>> {
        assert_eq!(object.version_id, "immutable-version");
        Ok(self
            .final_objects
            .lock()
            .unwrap()
            .get(&object.key)
            .unwrap()
            .clone())
    }
    async fn download(&self, _: &StoredAttachment, _: &str) -> AppResult<SignedAttachmentUrl> {
        Ok(signed())
    }
    async fn cache_download(&self, _: &str, _: Vec<u8>, _: &str) -> AppResult<SignedAttachmentUrl> {
        Ok(signed())
    }
}

fn compose(attachments: Vec<OutboundAttachmentInput>) -> ComposeMessageRequest {
    ComposeMessageRequest {
        from_address: "support@ahara.io".into(),
        to: vec!["person@example.test".into()],
        cc: vec![],
        bcc: vec![],
        subject: "Files".into(),
        body_text: "Attached notes".into(),
        attachments,
    }
}

#[tokio::test]
async fn postgres_uploads_enforce_ownership_limits_and_pin_queued_mail() {
    let (_container, pool) = database().await;
    let storage = Arc::new(Storage::default());
    let uploads = PgAttachmentUploadService::new(pool.clone(), storage.clone());
    let bytes = vec![b'x'; MAX_OUTBOUND_ATTACHMENT_BYTES];
    let grant = uploads.create("owner", request(&bytes)).await.unwrap();
    let references = [UploadedAttachmentRef {
        upload_id: grant.upload_id,
    }];
    assert!(uploads.resolve("another-user", &references).await.is_err());
    assert!(uploads.resolve("owner", &references).await.is_err());
    let key = format!("attachments/staging/{}", grant.upload_id);
    storage
        .stage
        .lock()
        .unwrap()
        .insert(key.clone(), vec![b'z'; bytes.len()]);
    assert!(uploads.resolve("owner", &references).await.is_err());
    storage
        .stage
        .lock()
        .unwrap()
        .insert(key.clone(), bytes.clone());
    let attachments = uploads.resolve("owner", &references).await.unwrap();
    let object = attachments[0].storage.clone().unwrap();
    storage
        .stage
        .lock()
        .unwrap()
        .insert(key, b"later overwrite".to_vec());
    let again = uploads.resolve("owner", &references).await.unwrap();
    assert_eq!(again[0].storage, Some(object.clone()));
    let outbound =
        PgOutboundService::new(pool.clone(), "ahara.io").with_attachment_storage(storage.clone());
    let queued = outbound
        .compose_message(compose(attachments))
        .await
        .unwrap();
    let detail = outbound
        .get_outbound_message(&queued.message_id)
        .await
        .unwrap();
    assert_eq!(detail.attachments[0].storage, Some(object));
    assert!(
        !serde_json::to_string(&detail)
            .unwrap()
            .contains("content_base64")
    );
    let inline: Option<String> =
        sqlx::query_scalar("SELECT content_base64 FROM outbound_attachment_payloads")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(inline.is_none());
    let work = outbound.claim_due_work("test", 1).await.unwrap();
    assert_eq!(work.len(), 1);
    assert!(work[0].raw_message.len() < 40 * 1024 * 1024);
    let mime = mailparse::parse_mail(&work[0].raw_message).unwrap();
    assert_eq!(mime.subparts[1].get_body_raw().unwrap(), bytes);
    outbound
        .mark_send_retry(&work[0].work_id, "temporary error")
        .await
        .unwrap();
    sqlx::query("UPDATE outbound_work SET next_attempt_at = now() - interval '1 minute'")
        .execute(&pool)
        .await
        .unwrap();
    let retry = outbound.claim_due_work("retry", 1).await.unwrap();
    let mime = mailparse::parse_mail(&retry[0].raw_message).unwrap();
    assert_eq!(
        mime.subparts[1].get_body_raw().unwrap().len(),
        MAX_OUTBOUND_ATTACHMENT_BYTES
    );
    let small = uploads.create("owner", request(b"y")).await.unwrap();
    let too_many_bytes = [
        references[0].clone(),
        UploadedAttachmentRef {
            upload_id: small.upload_id,
        },
    ];
    assert!(uploads.resolve("owner", &too_many_bytes).await.is_err());
    assert!(
        uploads
            .create(
                "owner",
                request(&vec![0; MAX_OUTBOUND_ATTACHMENT_BYTES + 1])
            )
            .await
            .is_err()
    );
    sqlx::query("UPDATE attachment_uploads SET expires_at = now() - interval '1 hour'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(uploads.resolve("owner", &references).await.is_err());
}

#[tokio::test]
async fn legacy_database_attachment_payloads_still_send() {
    let (_container, pool) = database().await;
    let outbound = PgOutboundService::new(pool, "ahara.io");
    let queued = outbound
        .compose_message(compose(vec![OutboundAttachmentInput {
            filename: "legacy.txt".into(),
            content_type: "text/plain".into(),
            content_base64: "bGVnYWN5".into(),
            storage: None,
        }]))
        .await
        .unwrap();
    let detail = outbound
        .get_outbound_message(&queued.message_id)
        .await
        .unwrap();
    assert_eq!(detail.attachments[0].content_base64, "bGVnYWN5");
    let work = outbound.claim_due_work("legacy", 1).await.unwrap();
    let mime = mailparse::parse_mail(&work[0].raw_message).unwrap();
    assert_eq!(mime.subparts[1].get_body_raw().unwrap(), b"legacy");
}
