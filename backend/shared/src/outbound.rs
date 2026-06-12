use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc2822;
use uuid::Uuid;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::inbound::threading::{normalize_subject, participants_json};
use crate::mailbox::{DEFAULT_SNIPPET_CHARS, body_snippet};
use crate::ports::{MailSender, OutboundMailRequest};
use crate::routing::parse_route;

pub const ENQUEUE_LIMIT_PER_FROM_PER_HOUR: i64 = 60;
pub const WORKER_BATCH_LIMIT: i64 = 25;
pub const MAX_SEND_ATTEMPTS: i32 = 5;
pub const RETRY_BACKOFF_MINUTES: [i64; 4] = [5, 30, 120, 480];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComposeMessageRequest {
    pub from_address: String,
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_text: String,
}

impl ComposeMessageRequest {
    pub fn validate(&self, configured_domain: &str) -> AppResult<ValidatedOutboundMessage> {
        Ok(ValidatedOutboundMessage {
            from: normalize_outbound_from_address(&self.from_address, configured_domain)?,
            recipients: normalize_outbound_recipients(&self.to, &self.cc, &self.bcc)?,
            subject: normalize_header_text("subject", &self.subject)?,
            body_text: self.body_text.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplyMessageRequest {
    pub from_address: String,
    #[serde(default)]
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    pub body_text: String,
}

impl ReplyMessageRequest {
    pub fn validate(&self, configured_domain: &str) -> AppResult<ValidatedOutboundMessage> {
        Ok(ValidatedOutboundMessage {
            from: normalize_outbound_from_address(&self.from_address, configured_domain)?,
            recipients: normalize_outbound_recipients(&self.to, &self.cc, &self.bcc)?,
            subject: String::new(),
            body_text: self.body_text.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardMessageRequest {
    pub from_address: String,
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    #[serde(default)]
    pub note_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnqueueForwardRequest {
    pub source_message_id: String,
    pub source_thread_id: Option<String>,
    pub source_rfc_message_id: Option<String>,
    pub source_reference_ids: Vec<String>,
    pub forwarding_rule_id: String,
    pub from_address: String,
    pub target_address: String,
    pub original_from_address: String,
    pub original_subject: String,
    pub original_body_text: String,
}

impl ForwardMessageRequest {
    pub fn validate(&self, configured_domain: &str) -> AppResult<ValidatedOutboundMessage> {
        Ok(ValidatedOutboundMessage {
            from: normalize_outbound_from_address(&self.from_address, configured_domain)?,
            recipients: normalize_outbound_recipients(&self.to, &self.cc, &self.bcc)?,
            subject: String::new(),
            body_text: self.note_text.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundMessageQueued {
    pub message_id: String,
    pub work_id: String,
    pub rfc_message_id: String,
    pub status: OutboundMessageStatus,
    pub recipients: Vec<OutboundRecipient>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedOutboundMessage {
    pub from: OutboundAddress,
    pub recipients: Vec<OutboundRecipient>,
    pub subject: String,
    pub body_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundAddress {
    pub address: String,
    pub address_normalized: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundRecipient {
    pub kind: OutboundRecipientKind,
    pub address: String,
    pub address_normalized: String,
    pub display_name: String,
    pub position: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutboundRecipientKind {
    To,
    Cc,
    Bcc,
}

impl OutboundRecipientKind {
    pub fn as_db_value(self) -> &'static str {
        match self {
            Self::To => "to",
            Self::Cc => "cc",
            Self::Bcc => "bcc",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "to" => Some(Self::To),
            "cc" => Some(Self::Cc),
            "bcc" => Some(Self::Bcc),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutboundMessageStatus {
    Queued,
    Sending,
    Sent,
    Failed,
    Bounced,
    Complained,
}

impl OutboundMessageStatus {
    pub fn as_db_value(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Failed => "failed",
            Self::Bounced => "bounced",
            Self::Complained => "complained",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "queued" => Some(Self::Queued),
            "sending" => Some(Self::Sending),
            "sent" => Some(Self::Sent),
            "failed" => Some(Self::Failed),
            "bounced" => Some(Self::Bounced),
            "complained" => Some(Self::Complained),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundMimeMessage {
    pub from_address: String,
    pub to_addresses: Vec<String>,
    pub cc_addresses: Vec<String>,
    pub bcc_addresses: Vec<String>,
    pub subject: String,
    pub body_text: String,
    pub message_id: String,
    pub date: String,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    pub reply_to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardedMessageSource {
    pub from_address: String,
    pub subject: String,
    pub body_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundMessageDetail {
    pub id: String,
    pub source_message_id: Option<String>,
    pub thread_id: Option<String>,
    pub rfc_message_id: String,
    pub in_reply_to: Option<String>,
    pub reference_ids: Vec<String>,
    pub status: OutboundMessageStatus,
    pub from_address: String,
    pub from_address_normalized: String,
    pub subject: String,
    pub body_text: String,
    pub recipients: Vec<OutboundRecipient>,
    pub last_error: Option<String>,
    pub sent_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundMessageSummary {
    pub id: String,
    pub thread_id: Option<String>,
    pub status: OutboundMessageStatus,
    pub from_address: String,
    pub subject: String,
    pub snippet: String,
    pub primary_recipient: Option<String>,
    pub recipient_count: i64,
    pub last_error: Option<String>,
    pub sent_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedOutboundWork {
    pub work_id: String,
    pub message_id: String,
    pub source_message_id: Option<String>,
    pub attempt_count: i32,
    pub from_address: String,
    pub to_addresses: Vec<String>,
    pub raw_message: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OutboundSendSummary {
    pub claimed: usize,
    pub sent: usize,
    pub retried: usize,
    pub failed: usize,
    pub suppressed: usize,
}

#[async_trait]
pub trait OutboundService: Send + Sync {
    async fn compose_message(
        &self,
        request: ComposeMessageRequest,
    ) -> AppResult<OutboundMessageQueued>;

    async fn reply_to_message(
        &self,
        source_message_id: &str,
        request: ReplyMessageRequest,
    ) -> AppResult<OutboundMessageQueued>;

    async fn enqueue_forward(
        &self,
        request: EnqueueForwardRequest,
    ) -> AppResult<OutboundMessageQueued>;

    async fn list_outbound_messages(&self) -> AppResult<Vec<OutboundMessageSummary>>;

    async fn get_outbound_message(&self, message_id: &str) -> AppResult<OutboundMessageDetail>;

    async fn claim_due_work(
        &self,
        worker_id: &str,
        limit: i64,
    ) -> AppResult<Vec<ClaimedOutboundWork>>;

    async fn suppressed_recipient(&self, addresses: &[String]) -> AppResult<Option<String>>;

    async fn mark_send_success(&self, work_id: &str, provider_message_id: &str) -> AppResult<()>;

    async fn mark_send_retry(&self, work_id: &str, error_message: &str) -> AppResult<()>;

    async fn mark_send_permanent_failure(
        &self,
        work_id: &str,
        error_message: &str,
    ) -> AppResult<()>;
}

#[derive(Clone)]
pub struct OutboundSendWorker {
    outbound: Arc<dyn OutboundService>,
    mail_sender: Arc<dyn MailSender>,
    worker_id: String,
    batch_limit: i64,
}

impl OutboundSendWorker {
    pub fn new(
        outbound: Arc<dyn OutboundService>,
        mail_sender: Arc<dyn MailSender>,
        worker_id: impl Into<String>,
    ) -> Self {
        Self {
            outbound,
            mail_sender,
            worker_id: worker_id.into(),
            batch_limit: WORKER_BATCH_LIMIT,
        }
    }

    pub fn with_batch_limit(mut self, batch_limit: i64) -> Self {
        self.batch_limit = batch_limit.clamp(1, WORKER_BATCH_LIMIT);
        self
    }

    pub async fn run_once(&self) -> AppResult<OutboundSendSummary> {
        let work = self
            .outbound
            .claim_due_work(&self.worker_id, self.batch_limit)
            .await?;
        let mut summary = OutboundSendSummary {
            claimed: work.len(),
            ..OutboundSendSummary::default()
        };

        for item in work {
            if let Some(address) = self
                .outbound
                .suppressed_recipient(&item.to_addresses)
                .await?
            {
                self.outbound
                    .mark_send_permanent_failure(
                        &item.work_id,
                        &format!("recipient {address} is suppressed"),
                    )
                    .await?;
                summary.failed += 1;
                summary.suppressed += 1;
                continue;
            }

            let send_result = self
                .mail_sender
                .send_mail(OutboundMailRequest {
                    from_address: item.from_address.clone(),
                    to_addresses: item.to_addresses.clone(),
                    raw_message: item.raw_message.clone(),
                })
                .await;
            match send_result {
                Ok(response) => {
                    self.outbound
                        .mark_send_success(&item.work_id, &response.provider_message_id)
                        .await?;
                    summary.sent += 1;
                }
                Err(err) if item.attempt_count >= MAX_SEND_ATTEMPTS => {
                    self.outbound
                        .mark_send_permanent_failure(&item.work_id, &err.public_message())
                        .await?;
                    summary.failed += 1;
                }
                Err(err) => {
                    self.outbound
                        .mark_send_retry(&item.work_id, &err.public_message())
                        .await?;
                    summary.retried += 1;
                }
            }
        }

        Ok(summary)
    }
}

#[derive(Debug, Clone)]
pub struct PgOutboundService {
    pool: DbPool,
    configured_domain: String,
}

impl PgOutboundService {
    pub fn new(pool: DbPool, configured_domain: impl Into<String>) -> Self {
        Self {
            pool,
            configured_domain: configured_domain.into(),
        }
    }
}

#[async_trait]
impl OutboundService for PgOutboundService {
    async fn compose_message(
        &self,
        request: ComposeMessageRequest,
    ) -> AppResult<OutboundMessageQueued> {
        let validated = request.validate(&self.configured_domain)?;
        self.enqueue_message(EnqueueOutboundMessage {
            source_message_id: None,
            thread_id: None,
            from: validated.from,
            recipients: validated.recipients,
            subject: validated.subject,
            body_text: validated.body_text,
            in_reply_to: None,
            reference_ids: Vec::new(),
            idempotency_key: format!("compose:{}", Uuid::new_v4()),
            reply_to: None,
        })
        .await
    }

    async fn reply_to_message(
        &self,
        source_message_id: &str,
        request: ReplyMessageRequest,
    ) -> AppResult<OutboundMessageQueued> {
        let source_message_id = parse_uuid(source_message_id, "source message id")?;
        let source = self.fetch_reply_source(source_message_id).await?;
        let request = reply_request_with_default_recipient(request, &source.from_address);
        let mut validated = request.validate(&self.configured_domain)?;
        validated.subject = reply_subject(&source.subject)?;
        let reference_ids =
            build_reply_references(source.rfc_message_id.as_deref(), &source.reference_ids)?;

        self.enqueue_message(EnqueueOutboundMessage {
            source_message_id: Some(source.id),
            thread_id: source.thread_id,
            from: validated.from,
            recipients: validated.recipients,
            subject: validated.subject,
            body_text: validated.body_text,
            in_reply_to: source.rfc_message_id,
            reference_ids,
            idempotency_key: format!("reply:{source_message_id}:{}", Uuid::new_v4()),
            reply_to: None,
        })
        .await
    }

    async fn enqueue_forward(
        &self,
        request: EnqueueForwardRequest,
    ) -> AppResult<OutboundMessageQueued> {
        let source_message_id = parse_uuid(&request.source_message_id, "source message id")?;
        let source_thread_id = request
            .source_thread_id
            .as_deref()
            .map(|value| parse_uuid(value, "source thread id"))
            .transpose()?;
        let from = normalize_outbound_from_address(&request.from_address, &self.configured_domain)?;
        let recipients =
            normalize_outbound_recipients(std::slice::from_ref(&request.target_address), &[], &[])?;
        let subject = forward_subject(&request.original_subject)?;
        let body_text = build_forward_body(
            &ForwardedMessageSource {
                from_address: request.original_from_address.clone(),
                subject: request.original_subject.clone(),
                body_text: request.original_body_text.clone(),
            },
            "",
        );

        let reference_ids = build_reply_references(
            request.source_rfc_message_id.as_deref(),
            &request.source_reference_ids,
        )?;

        self.enqueue_message(EnqueueOutboundMessage {
            source_message_id: Some(source_message_id),
            thread_id: source_thread_id,
            from,
            recipients,
            subject,
            body_text,
            in_reply_to: request.source_rfc_message_id,
            reference_ids,
            idempotency_key: format!(
                "forward:{}:{}",
                request.source_message_id, request.forwarding_rule_id
            ),
            reply_to: Some(request.original_from_address),
        })
        .await
    }

    async fn list_outbound_messages(&self) -> AppResult<Vec<OutboundMessageSummary>> {
        self.fetch_outbound_message_summaries().await
    }

    async fn get_outbound_message(&self, message_id: &str) -> AppResult<OutboundMessageDetail> {
        let message_id = parse_uuid(message_id, "message id")?;
        self.fetch_outbound_message_detail(message_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("outbound message {message_id}")))
    }

    async fn claim_due_work(
        &self,
        worker_id: &str,
        limit: i64,
    ) -> AppResult<Vec<ClaimedOutboundWork>> {
        if worker_id.trim().is_empty() {
            return Err(AppError::Validation("worker id is required".to_string()));
        }
        let limit = limit.clamp(1, WORKER_BATCH_LIMIT);
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;

        let claimed: Vec<ClaimedWorkRow> = sqlx::query_as(
            "WITH due AS (
                 SELECT outbound_work.id
                 FROM outbound_work
                 JOIN messages ON messages.id = outbound_work.message_id
                 WHERE outbound_work.status = 'queued'
                   AND outbound_work.next_attempt_at <= now()
                   AND messages.direction = 'outbound'
                   AND messages.status = 'queued'
                 ORDER BY outbound_work.next_attempt_at ASC, outbound_work.created_at ASC
                 LIMIT $1
                 FOR UPDATE SKIP LOCKED
             )
             UPDATE outbound_work
             SET status = 'sending',
                 attempt_count = outbound_work.attempt_count + 1,
                 locked_at = now(),
                 locked_by = $2,
                 updated_at = now()
             FROM due
             WHERE outbound_work.id = due.id
             RETURNING outbound_work.id,
                       outbound_work.message_id,
                       outbound_work.source_message_id,
                       outbound_work.attempt_count",
        )
        .bind(limit)
        .bind(worker_id.trim())
        .fetch_all(&mut *tx)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        let mut work = Vec::with_capacity(claimed.len());
        for row in claimed {
            sqlx::query(
                "UPDATE messages
                 SET status = 'sending',
                     send_attempt_count = $2,
                     updated_at = now()
                 WHERE id = $1",
            )
            .bind(row.message_id)
            .bind(row.attempt_count)
            .execute(&mut *tx)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;

            work.push(fetch_claimed_work_item(&mut tx, row).await?);
        }

        tx.commit()
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        Ok(work)
    }

    async fn suppressed_recipient(&self, addresses: &[String]) -> AppResult<Option<String>> {
        let normalized = addresses
            .iter()
            .map(|address| normalize_email_address(address))
            .collect::<AppResult<Vec<_>>>()?;
        let row: Option<SuppressionRow> = sqlx::query_as(
            "SELECT address
             FROM suppressions
             WHERE address_normalized = ANY($1)
             LIMIT 1",
        )
        .bind(&normalized)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        Ok(row.map(|row| row.address))
    }

    async fn mark_send_success(&self, work_id: &str, provider_message_id: &str) -> AppResult<()> {
        let work_id = parse_uuid(work_id, "work id")?;
        let provider_message_id =
            normalize_header_text("provider message id", provider_message_id)?;
        if provider_message_id.is_empty() {
            return Err(AppError::Validation(
                "provider message id is required".to_string(),
            ));
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        let row: Option<MessageIdRow> = sqlx::query_as(
            "UPDATE outbound_work
             SET status = 'sent',
                 locked_at = NULL,
                 locked_by = NULL,
                 last_error = NULL,
                 updated_at = now()
             WHERE id = $1
               AND status = 'sending'
             RETURNING message_id AS id",
        )
        .bind(work_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;
        let row = row.ok_or_else(|| AppError::NotFound(format!("outbound work {work_id}")))?;

        sqlx::query(
            "UPDATE messages
             SET status = 'sent',
                 ses_message_id = $2,
                 sent_at = now(),
                 next_retry_at = NULL,
                 last_error = NULL,
                 updated_at = now()
             WHERE id = $1",
        )
        .bind(row.id)
        .bind(provider_message_id)
        .execute(&mut *tx)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        tx.commit()
            .await
            .map_err(|err| AppError::Database(err.to_string()))
    }

    async fn mark_send_retry(&self, work_id: &str, error_message: &str) -> AppResult<()> {
        let work_id = parse_uuid(work_id, "work id")?;
        let error_message = normalize_error_message(error_message);
        let row: WorkAttemptRow = sqlx::query_as(
            "SELECT message_id AS id, attempt_count
             FROM outbound_work
             WHERE id = $1
               AND status = 'sending'",
        )
        .bind(work_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("outbound work {work_id}")))?;

        if row.attempt_count >= MAX_SEND_ATTEMPTS {
            return self
                .mark_send_permanent_failure(&work_id.to_string(), &error_message)
                .await;
        }

        let backoff_minutes = RETRY_BACKOFF_MINUTES
            .get(row.attempt_count.saturating_sub(1) as usize)
            .copied()
            .unwrap_or(*RETRY_BACKOFF_MINUTES.last().unwrap());
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;

        sqlx::query(
            "UPDATE outbound_work
             SET status = 'queued',
                 next_attempt_at = now() + ($2::bigint * interval '1 minute'),
                 locked_at = NULL,
                 locked_by = NULL,
                 last_error = $3,
                 updated_at = now()
             WHERE id = $1",
        )
        .bind(work_id)
        .bind(backoff_minutes)
        .bind(&error_message)
        .execute(&mut *tx)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        sqlx::query(
            "UPDATE messages
             SET status = 'queued',
                 next_retry_at = now() + ($2::bigint * interval '1 minute'),
                 last_error = $3,
                 updated_at = now()
             WHERE id = $1",
        )
        .bind(row.id)
        .bind(backoff_minutes)
        .bind(&error_message)
        .execute(&mut *tx)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        tx.commit()
            .await
            .map_err(|err| AppError::Database(err.to_string()))
    }

    async fn mark_send_permanent_failure(
        &self,
        work_id: &str,
        error_message: &str,
    ) -> AppResult<()> {
        let work_id = parse_uuid(work_id, "work id")?;
        let error_message = normalize_error_message(error_message);
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        let row: Option<MessageIdRow> = sqlx::query_as(
            "UPDATE outbound_work
             SET status = 'failed',
                 locked_at = NULL,
                 locked_by = NULL,
                 last_error = $2,
                 updated_at = now()
             WHERE id = $1
               AND status IN ('queued', 'sending')
             RETURNING message_id AS id",
        )
        .bind(work_id)
        .bind(&error_message)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;
        let row = row.ok_or_else(|| AppError::NotFound(format!("outbound work {work_id}")))?;

        sqlx::query(
            "UPDATE messages
             SET status = 'failed',
                 next_retry_at = NULL,
                 last_error = $2,
                 updated_at = now()
             WHERE id = $1",
        )
        .bind(row.id)
        .bind(&error_message)
        .execute(&mut *tx)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        tx.commit()
            .await
            .map_err(|err| AppError::Database(err.to_string()))
    }
}

impl PgOutboundService {
    async fn enqueue_message(
        &self,
        request: EnqueueOutboundMessage,
    ) -> AppResult<OutboundMessageQueued> {
        if let Some(existing) = self
            .fetch_outbound_message_by_idempotency(&request.idempotency_key)
            .await?
        {
            return Ok(existing);
        }
        reject_suppressed_recipients(&self.pool, &request.recipients).await?;
        enforce_rate_limit(&self.pool, &request.from.address_normalized).await?;

        let rfc_message_id = build_message_id(&self.configured_domain)?;
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        let thread_id = match request.thread_id {
            Some(thread_id) => {
                merge_thread_participants(&mut tx, thread_id, &request).await?;
                thread_id
            }
            None => insert_outbound_thread(&mut tx, &request).await?,
        };

        let message_id =
            insert_outbound_message(&mut tx, thread_id, &rfc_message_id, &request).await?;
        insert_outbound_recipients(&mut tx, message_id, &request.recipients).await?;
        let work_id = insert_outbound_work(&mut tx, message_id, &request).await?;

        tx.commit()
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;

        Ok(OutboundMessageQueued {
            message_id: message_id.to_string(),
            work_id: work_id.to_string(),
            rfc_message_id,
            status: OutboundMessageStatus::Queued,
            recipients: request.recipients,
        })
    }

    async fn fetch_reply_source(&self, source_message_id: Uuid) -> AppResult<ReplySourceRow> {
        sqlx::query_as(
            "SELECT id,
                    thread_id,
                    rfc_message_id,
                    reference_ids,
                    from_address,
                    subject
             FROM messages
             WHERE id = $1
               AND direction = 'inbound'
               AND security_disposition = 'accepted'
               AND status = 'received'",
        )
        .bind(source_message_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("source message {source_message_id}")))
    }

    async fn fetch_outbound_message_summaries(&self) -> AppResult<Vec<OutboundMessageSummary>> {
        let rows: Vec<OutboundMessageSummaryRow> = sqlx::query_as(
            "SELECT messages.id,
                    messages.thread_id,
                    messages.status,
                    messages.from_address,
                    messages.subject,
                    messages.body_text,
                    (
                        SELECT recipients.address
                        FROM recipients
                        WHERE recipients.message_id = messages.id
                        ORDER BY
                            CASE recipients.kind
                                WHEN 'to' THEN 0
                                WHEN 'cc' THEN 1
                                WHEN 'bcc' THEN 2
                                ELSE 3
                            END,
                            recipients.position ASC,
                            recipients.id ASC
                        LIMIT 1
                    ) AS primary_recipient,
                    (
                        SELECT count(*)
                        FROM recipients
                        WHERE recipients.message_id = messages.id
                    ) AS recipient_count,
                    messages.last_error,
                    messages.sent_at::text AS sent_at,
                    messages.created_at::text AS created_at
             FROM messages
             WHERE messages.direction = 'outbound'
             ORDER BY COALESCE(messages.sent_at, messages.created_at) DESC,
                      messages.created_at DESC,
                      messages.id DESC
             LIMIT 100",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn fetch_outbound_message_detail(
        &self,
        message_id: Uuid,
    ) -> AppResult<Option<OutboundMessageDetail>> {
        let row: Option<OutboundMessageRow> = sqlx::query_as(
            "SELECT messages.id,
                    outbound_work.source_message_id,
                    messages.thread_id,
                    messages.rfc_message_id,
                    messages.in_reply_to,
                    messages.reference_ids,
                    messages.status,
                    messages.from_address,
                    messages.from_address_normalized,
                    messages.subject,
                    messages.body_text,
                    messages.last_error,
                    messages.sent_at::text AS sent_at,
                    messages.created_at::text AS created_at
             FROM messages
             LEFT JOIN outbound_work ON outbound_work.message_id = messages.id
             WHERE messages.id = $1
               AND messages.direction = 'outbound'",
        )
        .bind(message_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };
        let recipients = fetch_outbound_recipients(&self.pool, row.id).await?;
        row.into_detail(recipients).map(Some)
    }

    async fn fetch_outbound_message_by_idempotency(
        &self,
        idempotency_key: &str,
    ) -> AppResult<Option<OutboundMessageQueued>> {
        let row: Option<OutboundQueuedRow> = sqlx::query_as(
            "SELECT messages.id AS message_id,
                    outbound_work.id AS work_id,
                    messages.rfc_message_id,
                    outbound_work.status
             FROM outbound_work
             JOIN messages ON messages.id = outbound_work.message_id
             WHERE outbound_work.idempotency_key = $1",
        )
        .bind(idempotency_key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };
        let recipients = fetch_outbound_recipients(&self.pool, row.message_id).await?;
        row.into_queued(recipients).map(Some)
    }
}

#[derive(Debug, Clone)]
struct EnqueueOutboundMessage {
    source_message_id: Option<Uuid>,
    thread_id: Option<Uuid>,
    from: OutboundAddress,
    recipients: Vec<OutboundRecipient>,
    subject: String,
    body_text: String,
    in_reply_to: Option<String>,
    reference_ids: Vec<String>,
    idempotency_key: String,
    reply_to: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryOutboundService {
    configured_domain: String,
    state: Arc<Mutex<InMemoryOutboundState>>,
}

impl InMemoryOutboundService {
    pub fn new(configured_domain: impl Into<String>) -> Self {
        Self {
            configured_domain: configured_domain.into(),
            state: Arc::new(Mutex::new(InMemoryOutboundState::default())),
        }
    }

    pub fn suppress_address(&self, address: impl Into<String>) {
        self.state
            .lock()
            .unwrap()
            .suppressions
            .insert(address.into().to_ascii_lowercase());
    }

    pub fn seed_reply_source(&self, source: InMemoryReplySource) {
        self.state
            .lock()
            .unwrap()
            .reply_sources
            .insert(source.id, source);
    }
}

#[async_trait]
impl OutboundService for InMemoryOutboundService {
    async fn compose_message(
        &self,
        request: ComposeMessageRequest,
    ) -> AppResult<OutboundMessageQueued> {
        let validated = request.validate(&self.configured_domain)?;
        self.enqueue_memory_message(EnqueueOutboundMessage {
            source_message_id: None,
            thread_id: None,
            from: validated.from,
            recipients: validated.recipients,
            subject: validated.subject,
            body_text: validated.body_text,
            in_reply_to: None,
            reference_ids: Vec::new(),
            idempotency_key: format!("compose:{}", Uuid::new_v4()),
            reply_to: None,
        })
    }

    async fn reply_to_message(
        &self,
        source_message_id: &str,
        request: ReplyMessageRequest,
    ) -> AppResult<OutboundMessageQueued> {
        let source_message_id = parse_uuid(source_message_id, "source message id")?;
        let source = self
            .state
            .lock()
            .unwrap()
            .reply_sources
            .get(&source_message_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("source message {source_message_id}")))?;
        let request = reply_request_with_default_recipient(request, &source.from_address);
        let mut validated = request.validate(&self.configured_domain)?;
        validated.subject = reply_subject(&source.subject)?;

        self.enqueue_memory_message(EnqueueOutboundMessage {
            source_message_id: Some(source.id),
            thread_id: source.thread_id,
            from: validated.from,
            recipients: validated.recipients,
            subject: validated.subject,
            body_text: validated.body_text,
            in_reply_to: source.rfc_message_id.clone(),
            reference_ids: build_reply_references(
                source.rfc_message_id.as_deref(),
                &source.reference_ids,
            )?,
            idempotency_key: format!("reply:{source_message_id}:{}", Uuid::new_v4()),
            reply_to: None,
        })
    }

    async fn enqueue_forward(
        &self,
        request: EnqueueForwardRequest,
    ) -> AppResult<OutboundMessageQueued> {
        let source_message_id = parse_uuid(&request.source_message_id, "source message id")?;
        let source_thread_id = request
            .source_thread_id
            .as_deref()
            .map(|value| parse_uuid(value, "source thread id"))
            .transpose()?;
        let from = normalize_outbound_from_address(&request.from_address, &self.configured_domain)?;
        let recipients =
            normalize_outbound_recipients(std::slice::from_ref(&request.target_address), &[], &[])?;
        let subject = forward_subject(&request.original_subject)?;
        let body_text = build_forward_body(
            &ForwardedMessageSource {
                from_address: request.original_from_address.clone(),
                subject: request.original_subject.clone(),
                body_text: request.original_body_text.clone(),
            },
            "",
        );
        let reference_ids = build_reply_references(
            request.source_rfc_message_id.as_deref(),
            &request.source_reference_ids,
        )?;

        self.enqueue_memory_message(EnqueueOutboundMessage {
            source_message_id: Some(source_message_id),
            thread_id: source_thread_id,
            from,
            recipients,
            subject,
            body_text,
            in_reply_to: request.source_rfc_message_id,
            reference_ids,
            idempotency_key: format!(
                "forward:{}:{}",
                request.source_message_id, request.forwarding_rule_id
            ),
            reply_to: Some(request.original_from_address),
        })
    }

    async fn get_outbound_message(&self, message_id: &str) -> AppResult<OutboundMessageDetail> {
        let message_id = parse_uuid(message_id, "message id")?;
        self.state
            .lock()
            .unwrap()
            .messages
            .get(&message_id)
            .cloned()
            .map(InMemoryOutboundRecord::into_detail)
            .ok_or_else(|| AppError::NotFound(format!("outbound message {message_id}")))
    }

    async fn list_outbound_messages(&self) -> AppResult<Vec<OutboundMessageSummary>> {
        let mut summaries = self
            .state
            .lock()
            .unwrap()
            .messages
            .values()
            .cloned()
            .map(InMemoryOutboundRecord::into_summary)
            .collect::<Vec<_>>();
        sort_outbound_summaries_desc(&mut summaries);
        Ok(summaries)
    }

    async fn claim_due_work(
        &self,
        worker_id: &str,
        limit: i64,
    ) -> AppResult<Vec<ClaimedOutboundWork>> {
        if worker_id.trim().is_empty() {
            return Err(AppError::Validation("worker id is required".to_string()));
        }
        let limit = limit.clamp(1, WORKER_BATCH_LIMIT) as usize;
        let mut state = self.state.lock().unwrap();
        let mut claimed = Vec::new();
        let work_ids = state
            .work_order
            .iter()
            .copied()
            .filter(|work_id| {
                state
                    .work
                    .get(work_id)
                    .is_some_and(|work| work.status == OutboundMessageStatus::Queued)
            })
            .take(limit)
            .collect::<Vec<_>>();

        for work_id in work_ids {
            let Some(work) = state.work.get_mut(&work_id) else {
                continue;
            };
            work.status = OutboundMessageStatus::Sending;
            work.attempt_count += 1;
            let work_snapshot = work.clone();
            let Some(message) = state.messages.get_mut(&work_snapshot.message_id) else {
                continue;
            };
            message.status = OutboundMessageStatus::Sending;
            message.send_attempt_count = work_snapshot.attempt_count;
            let message_snapshot = message.clone();
            claimed.push(claimed_memory_work(&work_snapshot, &message_snapshot)?);
        }

        Ok(claimed)
    }

    async fn suppressed_recipient(&self, addresses: &[String]) -> AppResult<Option<String>> {
        let state = self.state.lock().unwrap();
        for address in addresses {
            let normalized = normalize_email_address(address)?;
            if state.suppressions.contains(&normalized) {
                return Ok(Some(address.clone()));
            }
        }
        Ok(None)
    }

    async fn mark_send_success(&self, work_id: &str, provider_message_id: &str) -> AppResult<()> {
        let work_id = parse_uuid(work_id, "work id")?;
        let provider_message_id =
            normalize_header_text("provider message id", provider_message_id)?;
        if provider_message_id.is_empty() {
            return Err(AppError::Validation(
                "provider message id is required".to_string(),
            ));
        }
        let mut state = self.state.lock().unwrap();
        let message_id = {
            let work = state
                .work
                .get_mut(&work_id)
                .ok_or_else(|| AppError::NotFound(format!("outbound work {work_id}")))?;
            work.status = OutboundMessageStatus::Sent;
            work.message_id
        };
        let message = state
            .messages
            .get_mut(&message_id)
            .ok_or_else(|| AppError::NotFound(format!("outbound message {message_id}")))?;
        message.status = OutboundMessageStatus::Sent;
        message.ses_message_id = Some(provider_message_id);
        message.last_error = None;
        message.sent_at = Some("now".to_string());
        Ok(())
    }

    async fn mark_send_retry(&self, work_id: &str, error_message: &str) -> AppResult<()> {
        let work_id = parse_uuid(work_id, "work id")?;
        let error_message = normalize_error_message(error_message);
        let mut state = self.state.lock().unwrap();
        let work = state
            .work
            .get_mut(&work_id)
            .ok_or_else(|| AppError::NotFound(format!("outbound work {work_id}")))?;
        if work.attempt_count >= MAX_SEND_ATTEMPTS {
            work.status = OutboundMessageStatus::Failed;
            work.last_error = Some(error_message.clone());
            let message_id = work.message_id;
            let message = state
                .messages
                .get_mut(&message_id)
                .ok_or_else(|| AppError::NotFound(format!("outbound message {message_id}")))?;
            message.status = OutboundMessageStatus::Failed;
            message.last_error = Some(error_message);
            return Ok(());
        }
        work.status = OutboundMessageStatus::Queued;
        work.last_error = Some(error_message.clone());
        let message_id = work.message_id;
        let message = state
            .messages
            .get_mut(&message_id)
            .ok_or_else(|| AppError::NotFound(format!("outbound message {message_id}")))?;
        message.status = OutboundMessageStatus::Queued;
        message.last_error = Some(error_message);
        Ok(())
    }

    async fn mark_send_permanent_failure(
        &self,
        work_id: &str,
        error_message: &str,
    ) -> AppResult<()> {
        let work_id = parse_uuid(work_id, "work id")?;
        let error_message = normalize_error_message(error_message);
        let mut state = self.state.lock().unwrap();
        let work = state
            .work
            .get_mut(&work_id)
            .ok_or_else(|| AppError::NotFound(format!("outbound work {work_id}")))?;
        work.status = OutboundMessageStatus::Failed;
        work.last_error = Some(error_message.clone());
        let message_id = work.message_id;
        let message = state
            .messages
            .get_mut(&message_id)
            .ok_or_else(|| AppError::NotFound(format!("outbound message {message_id}")))?;
        message.status = OutboundMessageStatus::Failed;
        message.last_error = Some(error_message);
        Ok(())
    }
}

impl InMemoryOutboundService {
    fn enqueue_memory_message(
        &self,
        request: EnqueueOutboundMessage,
    ) -> AppResult<OutboundMessageQueued> {
        let mut state = self.state.lock().unwrap();
        if let Some(work) = state
            .work
            .values()
            .find(|work| work.idempotency_key == request.idempotency_key)
        {
            let message = state.messages.get(&work.message_id).ok_or_else(|| {
                AppError::NotFound(format!("outbound message {}", work.message_id))
            })?;
            return Ok(OutboundMessageQueued {
                message_id: message.id.to_string(),
                work_id: work.id.to_string(),
                rfc_message_id: message.rfc_message_id.clone(),
                status: work.status,
                recipients: message.recipients.clone(),
            });
        }
        if let Some(recipient) = request.recipients.iter().find(|recipient| {
            state
                .suppressions
                .contains(&recipient.address_normalized.to_ascii_lowercase())
        }) {
            return Err(AppError::Validation(format!(
                "recipient {} is suppressed",
                recipient.address
            )));
        }
        let from_count = state
            .messages
            .values()
            .filter(|message| message.from.address_normalized == request.from.address_normalized)
            .count();
        if from_count >= ENQUEUE_LIMIT_PER_FROM_PER_HOUR as usize {
            return Err(AppError::Validation(
                "outbound enqueue rate limit exceeded".to_string(),
            ));
        }

        let message_id = Uuid::new_v4();
        let work_id = Uuid::new_v4();
        let rfc_message_id = build_message_id(&self.configured_domain)?;
        let thread_id = request.thread_id.unwrap_or_else(Uuid::new_v4);
        let record = InMemoryOutboundRecord {
            id: message_id,
            source_message_id: request.source_message_id,
            thread_id: Some(thread_id),
            rfc_message_id: rfc_message_id.clone(),
            in_reply_to: request.in_reply_to,
            reference_ids: request.reference_ids,
            status: OutboundMessageStatus::Queued,
            from: request.from,
            subject: request.subject,
            body_text: request.body_text,
            recipients: request.recipients,
            last_error: None,
            ses_message_id: None,
            send_attempt_count: 0,
            sent_at: None,
            created_at: "now".to_string(),
            reply_to: request.reply_to,
        };
        let work = InMemoryOutboundWork {
            id: work_id,
            message_id,
            source_message_id: record.source_message_id,
            status: OutboundMessageStatus::Queued,
            attempt_count: 0,
            last_error: None,
            idempotency_key: request.idempotency_key,
        };

        state.work_order.push(work_id);
        state.work.insert(work_id, work);
        state.messages.insert(message_id, record.clone());

        Ok(OutboundMessageQueued {
            message_id: message_id.to_string(),
            work_id: work_id.to_string(),
            rfc_message_id,
            status: OutboundMessageStatus::Queued,
            recipients: record.recipients,
        })
    }
}

#[derive(Debug, Clone, Default)]
struct InMemoryOutboundState {
    messages: BTreeMap<Uuid, InMemoryOutboundRecord>,
    work: BTreeMap<Uuid, InMemoryOutboundWork>,
    work_order: Vec<Uuid>,
    reply_sources: BTreeMap<Uuid, InMemoryReplySource>,
    suppressions: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InMemoryReplySource {
    pub id: Uuid,
    pub thread_id: Option<Uuid>,
    pub rfc_message_id: Option<String>,
    pub reference_ids: Vec<String>,
    pub from_address: String,
    pub subject: String,
}

#[derive(Debug, Clone)]
struct InMemoryOutboundRecord {
    id: Uuid,
    source_message_id: Option<Uuid>,
    thread_id: Option<Uuid>,
    rfc_message_id: String,
    in_reply_to: Option<String>,
    reference_ids: Vec<String>,
    status: OutboundMessageStatus,
    from: OutboundAddress,
    subject: String,
    body_text: String,
    recipients: Vec<OutboundRecipient>,
    last_error: Option<String>,
    ses_message_id: Option<String>,
    send_attempt_count: i32,
    sent_at: Option<String>,
    created_at: String,
    reply_to: Option<String>,
}

impl InMemoryOutboundRecord {
    fn into_summary(self) -> OutboundMessageSummary {
        let primary_recipient = self
            .recipients
            .iter()
            .min_by_key(|recipient| {
                let kind_order = match recipient.kind {
                    OutboundRecipientKind::To => 0,
                    OutboundRecipientKind::Cc => 1,
                    OutboundRecipientKind::Bcc => 2,
                };
                (kind_order, recipient.position)
            })
            .map(|recipient| recipient.address.clone());
        OutboundMessageSummary {
            id: self.id.to_string(),
            thread_id: self.thread_id.map(|id| id.to_string()),
            status: self.status,
            from_address: self.from.address,
            subject: self.subject,
            snippet: body_snippet(&self.body_text, DEFAULT_SNIPPET_CHARS),
            primary_recipient,
            recipient_count: self.recipients.len() as i64,
            last_error: self.last_error,
            sent_at: self.sent_at,
            created_at: self.created_at,
        }
    }

    fn into_detail(self) -> OutboundMessageDetail {
        OutboundMessageDetail {
            id: self.id.to_string(),
            source_message_id: self.source_message_id.map(|id| id.to_string()),
            thread_id: self.thread_id.map(|id| id.to_string()),
            rfc_message_id: self.rfc_message_id,
            in_reply_to: self.in_reply_to,
            reference_ids: self.reference_ids,
            status: self.status,
            from_address: self.from.address,
            from_address_normalized: self.from.address_normalized,
            subject: self.subject,
            body_text: self.body_text,
            recipients: self.recipients,
            last_error: self.last_error,
            sent_at: self.sent_at,
            created_at: self.created_at,
        }
    }
}

fn sort_outbound_summaries_desc(messages: &mut [OutboundMessageSummary]) {
    messages.sort_by(|left, right| {
        right
            .sent_at
            .as_ref()
            .unwrap_or(&right.created_at)
            .cmp(left.sent_at.as_ref().unwrap_or(&left.created_at))
            .then_with(|| right.created_at.cmp(&left.created_at))
            .then_with(|| right.id.cmp(&left.id))
    });
}

#[derive(Debug, Clone)]
struct InMemoryOutboundWork {
    id: Uuid,
    message_id: Uuid,
    source_message_id: Option<Uuid>,
    status: OutboundMessageStatus,
    attempt_count: i32,
    last_error: Option<String>,
    idempotency_key: String,
}

async fn reject_suppressed_recipients(
    pool: &DbPool,
    recipients: &[OutboundRecipient],
) -> AppResult<()> {
    let normalized = recipients
        .iter()
        .map(|recipient| recipient.address_normalized.clone())
        .collect::<Vec<_>>();
    let suppressed: Option<SuppressionRow> = sqlx::query_as(
        "SELECT address
         FROM suppressions
         WHERE address_normalized = ANY($1)
         LIMIT 1",
    )
    .bind(&normalized)
    .fetch_optional(pool)
    .await
    .map_err(|err| AppError::Database(err.to_string()))?;

    if let Some(suppressed) = suppressed {
        return Err(AppError::Validation(format!(
            "recipient {} is suppressed",
            suppressed.address
        )));
    }

    Ok(())
}

async fn enforce_rate_limit(pool: &DbPool, from_address_normalized: &str) -> AppResult<()> {
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*)
         FROM messages
         WHERE direction = 'outbound'
           AND from_address_normalized = $1
           AND created_at >= now() - interval '1 hour'",
    )
    .bind(from_address_normalized)
    .fetch_one(pool)
    .await
    .map_err(|err| AppError::Database(err.to_string()))?;

    if count >= ENQUEUE_LIMIT_PER_FROM_PER_HOUR {
        return Err(AppError::Validation(
            "outbound enqueue rate limit exceeded".to_string(),
        ));
    }

    Ok(())
}

async fn insert_outbound_thread(
    tx: &mut Transaction<'_, Postgres>,
    request: &EnqueueOutboundMessage,
) -> AppResult<Uuid> {
    let participants = participants_json(&outbound_participants(request));
    let row: MessageIdRow = sqlx::query_as(
        "INSERT INTO threads (normalized_subject, participants, last_activity_at, message_count)
         VALUES ($1, $2, now(), 1)
         RETURNING id",
    )
    .bind(normalize_subject(&request.subject))
    .bind(participants)
    .fetch_one(&mut **tx)
    .await
    .map_err(|err| AppError::Database(err.to_string()))?;
    Ok(row.id)
}

async fn merge_thread_participants(
    tx: &mut Transaction<'_, Postgres>,
    thread_id: Uuid,
    request: &EnqueueOutboundMessage,
) -> AppResult<()> {
    let row: Option<ThreadParticipantsRow> = sqlx::query_as(
        "SELECT participants
         FROM threads
         WHERE id = $1",
    )
    .bind(thread_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|err| AppError::Database(err.to_string()))?;
    let row = row.ok_or_else(|| AppError::NotFound(format!("thread {thread_id}")))?;
    let mut participants = row
        .participants
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(ToString::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut seen = participants.iter().cloned().collect::<BTreeSet<_>>();
    for participant in outbound_participants(request) {
        if seen.insert(participant.clone()) {
            participants.push(participant);
        }
    }

    sqlx::query(
        "UPDATE threads
         SET participants = $2,
             last_activity_at = now(),
             message_count = message_count + 1,
             updated_at = now()
         WHERE id = $1",
    )
    .bind(thread_id)
    .bind(participants_json(&participants))
    .execute(&mut **tx)
    .await
    .map_err(|err| AppError::Database(err.to_string()))?;

    Ok(())
}

fn outbound_participants(request: &EnqueueOutboundMessage) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut participants = Vec::new();
    for address in std::iter::once(&request.from.address_normalized).chain(
        request
            .recipients
            .iter()
            .map(|recipient| &recipient.address_normalized),
    ) {
        if seen.insert(address.clone()) {
            participants.push(address.clone());
        }
    }
    participants
}

async fn insert_outbound_message(
    tx: &mut Transaction<'_, Postgres>,
    thread_id: Uuid,
    rfc_message_id: &str,
    request: &EnqueueOutboundMessage,
) -> AppResult<Uuid> {
    let row: MessageIdRow = sqlx::query_as(
        "INSERT INTO messages (
             direction, rfc_message_id, in_reply_to, reference_ids, thread_id,
             from_address, from_address_normalized, subject, body_text, message_date,
             security_disposition, status, has_attachments, attachment_count
         )
         VALUES (
             'outbound', $1, $2, $3, $4,
             $5, $6, $7, $8, now(),
             'accepted', 'queued', false, 0
         )
         RETURNING id",
    )
    .bind(rfc_message_id)
    .bind(&request.in_reply_to)
    .bind(&request.reference_ids)
    .bind(thread_id)
    .bind(&request.from.address)
    .bind(&request.from.address_normalized)
    .bind(&request.subject)
    .bind(&request.body_text)
    .fetch_one(&mut **tx)
    .await
    .map_err(|err| AppError::Database(err.to_string()))?;
    Ok(row.id)
}

async fn insert_outbound_recipients(
    tx: &mut Transaction<'_, Postgres>,
    message_id: Uuid,
    recipients: &[OutboundRecipient],
) -> AppResult<()> {
    for recipient in recipients {
        sqlx::query(
            "INSERT INTO recipients (
                 message_id, kind, address, address_normalized, display_name, position
             )
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(message_id)
        .bind(recipient.kind.as_db_value())
        .bind(&recipient.address)
        .bind(&recipient.address_normalized)
        .bind(&recipient.display_name)
        .bind(recipient.position)
        .execute(&mut **tx)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;
    }
    Ok(())
}

async fn insert_outbound_work(
    tx: &mut Transaction<'_, Postgres>,
    message_id: Uuid,
    request: &EnqueueOutboundMessage,
) -> AppResult<Uuid> {
    let row: MessageIdRow = sqlx::query_as(
        "INSERT INTO outbound_work (message_id, source_message_id, status, idempotency_key)
         VALUES ($1, $2, 'queued', $3)
         RETURNING id",
    )
    .bind(message_id)
    .bind(request.source_message_id)
    .bind(&request.idempotency_key)
    .fetch_one(&mut **tx)
    .await
    .map_err(|err| AppError::Database(err.to_string()))?;
    Ok(row.id)
}

async fn fetch_claimed_work_item(
    tx: &mut Transaction<'_, Postgres>,
    row: ClaimedWorkRow,
) -> AppResult<ClaimedOutboundWork> {
    let message: ClaimMessageRow = sqlx::query_as(
        "SELECT from_address,
                subject,
                body_text,
                rfc_message_id,
                in_reply_to,
                reference_ids
         FROM messages
         WHERE id = $1
           AND direction = 'outbound'",
    )
    .bind(row.message_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|err| AppError::Database(err.to_string()))?;
    let recipients = fetch_outbound_recipients_tx(tx, row.message_id).await?;
    let reply_to = match row.source_message_id {
        Some(source_message_id) => fetch_source_reply_to(tx, source_message_id).await?,
        None => None,
    };
    let to_addresses = recipients
        .iter()
        .filter(|recipient| recipient.kind == OutboundRecipientKind::To)
        .map(|recipient| recipient.address.clone())
        .collect::<Vec<_>>();
    let cc_addresses = recipients
        .iter()
        .filter(|recipient| recipient.kind == OutboundRecipientKind::Cc)
        .map(|recipient| recipient.address.clone())
        .collect::<Vec<_>>();
    let bcc_addresses = recipients
        .iter()
        .filter(|recipient| recipient.kind == OutboundRecipientKind::Bcc)
        .map(|recipient| recipient.address.clone())
        .collect::<Vec<_>>();
    let raw_message = build_outbound_mime(&OutboundMimeMessage {
        from_address: message.from_address.clone(),
        to_addresses,
        cc_addresses,
        bcc_addresses,
        subject: message.subject,
        body_text: message.body_text,
        message_id: message
            .rfc_message_id
            .ok_or_else(|| AppError::Internal("outbound message missing rfc id".to_string()))?,
        date: current_rfc2822_date()?,
        in_reply_to: message.in_reply_to,
        references: message.reference_ids,
        reply_to,
    })?;

    Ok(ClaimedOutboundWork {
        work_id: row.id.to_string(),
        message_id: row.message_id.to_string(),
        source_message_id: row.source_message_id.map(|id| id.to_string()),
        attempt_count: row.attempt_count,
        from_address: message.from_address,
        to_addresses: recipient_delivery_addresses(&recipients),
        raw_message,
    })
}

async fn fetch_source_reply_to(
    tx: &mut Transaction<'_, Postgres>,
    source_message_id: Uuid,
) -> AppResult<Option<String>> {
    let row: Option<ReplyToRow> = sqlx::query_as(
        "SELECT from_address
         FROM messages
         WHERE id = $1
           AND direction = 'inbound'",
    )
    .bind(source_message_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|err| AppError::Database(err.to_string()))?;
    Ok(row.map(|row| row.from_address))
}

async fn fetch_outbound_recipients(
    pool: &DbPool,
    message_id: Uuid,
) -> AppResult<Vec<OutboundRecipient>> {
    let rows: Vec<OutboundRecipientRow> = sqlx::query_as(
        "SELECT kind, address, address_normalized, display_name, position
         FROM recipients
         WHERE message_id = $1
         ORDER BY position ASC, id ASC",
    )
    .bind(message_id)
    .fetch_all(pool)
    .await
    .map_err(|err| AppError::Database(err.to_string()))?;
    rows.into_iter().map(TryInto::try_into).collect()
}

async fn fetch_outbound_recipients_tx(
    tx: &mut Transaction<'_, Postgres>,
    message_id: Uuid,
) -> AppResult<Vec<OutboundRecipient>> {
    let rows: Vec<OutboundRecipientRow> = sqlx::query_as(
        "SELECT kind, address, address_normalized, display_name, position
         FROM recipients
         WHERE message_id = $1
         ORDER BY position ASC, id ASC",
    )
    .bind(message_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|err| AppError::Database(err.to_string()))?;
    rows.into_iter().map(TryInto::try_into).collect()
}

fn claimed_memory_work(
    work: &InMemoryOutboundWork,
    message: &InMemoryOutboundRecord,
) -> AppResult<ClaimedOutboundWork> {
    let to_addresses = message
        .recipients
        .iter()
        .filter(|recipient| recipient.kind == OutboundRecipientKind::To)
        .map(|recipient| recipient.address.clone())
        .collect::<Vec<_>>();
    let cc_addresses = message
        .recipients
        .iter()
        .filter(|recipient| recipient.kind == OutboundRecipientKind::Cc)
        .map(|recipient| recipient.address.clone())
        .collect::<Vec<_>>();
    let bcc_addresses = message
        .recipients
        .iter()
        .filter(|recipient| recipient.kind == OutboundRecipientKind::Bcc)
        .map(|recipient| recipient.address.clone())
        .collect::<Vec<_>>();
    let raw_message = build_outbound_mime(&OutboundMimeMessage {
        from_address: message.from.address.clone(),
        to_addresses,
        cc_addresses,
        bcc_addresses,
        subject: message.subject.clone(),
        body_text: message.body_text.clone(),
        message_id: message.rfc_message_id.clone(),
        date: current_rfc2822_date()?,
        in_reply_to: message.in_reply_to.clone(),
        references: message.reference_ids.clone(),
        reply_to: message.reply_to.clone(),
    })?;

    Ok(ClaimedOutboundWork {
        work_id: work.id.to_string(),
        message_id: work.message_id.to_string(),
        source_message_id: work.source_message_id.map(|id| id.to_string()),
        attempt_count: work.attempt_count,
        from_address: message.from.address.clone(),
        to_addresses: recipient_delivery_addresses(&message.recipients),
        raw_message,
    })
}

fn reply_request_with_default_recipient(
    mut request: ReplyMessageRequest,
    fallback_address: &str,
) -> ReplyMessageRequest {
    if request.to.is_empty() && request.cc.is_empty() && request.bcc.is_empty() {
        request.to.push(fallback_address.to_string());
    }
    request
}

fn parse_uuid(value: &str, label: &str) -> AppResult<Uuid> {
    Uuid::parse_str(value).map_err(|_| AppError::Validation(format!("{label} must be a UUID")))
}

fn normalize_error_message(error_message: &str) -> String {
    let normalized = error_message
        .replace(['\r', '\n'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.len() > 500 {
        normalized.chars().take(500).collect()
    } else if normalized.is_empty() {
        "mail send failed".to_string()
    } else {
        normalized
    }
}

#[derive(Debug, sqlx::FromRow)]
struct SuppressionRow {
    address: String,
}

#[derive(Debug, sqlx::FromRow)]
struct MessageIdRow {
    id: Uuid,
}

#[derive(Debug, sqlx::FromRow)]
struct WorkAttemptRow {
    id: Uuid,
    attempt_count: i32,
}

#[derive(Debug, sqlx::FromRow)]
struct ThreadParticipantsRow {
    participants: Value,
}

#[derive(Debug, sqlx::FromRow)]
struct ClaimedWorkRow {
    id: Uuid,
    message_id: Uuid,
    source_message_id: Option<Uuid>,
    attempt_count: i32,
}

#[derive(Debug, sqlx::FromRow)]
struct ReplySourceRow {
    id: Uuid,
    thread_id: Option<Uuid>,
    rfc_message_id: Option<String>,
    reference_ids: Vec<String>,
    from_address: String,
    subject: String,
}

#[derive(Debug, sqlx::FromRow)]
struct OutboundMessageRow {
    id: Uuid,
    source_message_id: Option<Uuid>,
    thread_id: Option<Uuid>,
    rfc_message_id: Option<String>,
    in_reply_to: Option<String>,
    reference_ids: Vec<String>,
    status: String,
    from_address: String,
    from_address_normalized: String,
    subject: String,
    body_text: String,
    last_error: Option<String>,
    sent_at: Option<String>,
    created_at: String,
}

#[derive(Debug, sqlx::FromRow)]
struct OutboundMessageSummaryRow {
    id: Uuid,
    thread_id: Option<Uuid>,
    status: String,
    from_address: String,
    subject: String,
    body_text: String,
    primary_recipient: Option<String>,
    recipient_count: i64,
    last_error: Option<String>,
    sent_at: Option<String>,
    created_at: String,
}

#[derive(Debug, sqlx::FromRow)]
struct OutboundQueuedRow {
    message_id: Uuid,
    work_id: Uuid,
    rfc_message_id: Option<String>,
    status: String,
}

impl OutboundQueuedRow {
    fn into_queued(self, recipients: Vec<OutboundRecipient>) -> AppResult<OutboundMessageQueued> {
        Ok(OutboundMessageQueued {
            message_id: self.message_id.to_string(),
            work_id: self.work_id.to_string(),
            rfc_message_id: self
                .rfc_message_id
                .ok_or_else(|| AppError::Internal("outbound message missing rfc id".to_string()))?,
            status: OutboundMessageStatus::parse(&self.status).ok_or_else(|| {
                AppError::Internal(format!("unknown outbound status {}", self.status))
            })?,
            recipients,
        })
    }
}

impl TryFrom<OutboundMessageSummaryRow> for OutboundMessageSummary {
    type Error = AppError;

    fn try_from(row: OutboundMessageSummaryRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id.to_string(),
            thread_id: row.thread_id.map(|id| id.to_string()),
            status: OutboundMessageStatus::parse(&row.status).ok_or_else(|| {
                AppError::Internal(format!("unknown outbound status {}", row.status))
            })?,
            from_address: row.from_address,
            subject: row.subject,
            snippet: body_snippet(&row.body_text, DEFAULT_SNIPPET_CHARS),
            primary_recipient: row.primary_recipient,
            recipient_count: row.recipient_count,
            last_error: row.last_error,
            sent_at: row.sent_at,
            created_at: row.created_at,
        })
    }
}

impl OutboundMessageRow {
    fn into_detail(self, recipients: Vec<OutboundRecipient>) -> AppResult<OutboundMessageDetail> {
        Ok(OutboundMessageDetail {
            id: self.id.to_string(),
            source_message_id: self.source_message_id.map(|id| id.to_string()),
            thread_id: self.thread_id.map(|id| id.to_string()),
            rfc_message_id: self
                .rfc_message_id
                .ok_or_else(|| AppError::Internal("outbound message missing rfc id".to_string()))?,
            in_reply_to: self.in_reply_to,
            reference_ids: self.reference_ids,
            status: OutboundMessageStatus::parse(&self.status).ok_or_else(|| {
                AppError::Internal(format!("unknown outbound status {}", self.status))
            })?,
            from_address: self.from_address,
            from_address_normalized: self.from_address_normalized,
            subject: self.subject,
            body_text: self.body_text,
            recipients,
            last_error: self.last_error,
            sent_at: self.sent_at,
            created_at: self.created_at,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct ClaimMessageRow {
    from_address: String,
    subject: String,
    body_text: String,
    rfc_message_id: Option<String>,
    in_reply_to: Option<String>,
    reference_ids: Vec<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct ReplyToRow {
    from_address: String,
}

#[derive(Debug, sqlx::FromRow)]
struct OutboundRecipientRow {
    kind: String,
    address: String,
    address_normalized: String,
    display_name: String,
    position: i32,
}

impl TryFrom<OutboundRecipientRow> for OutboundRecipient {
    type Error = AppError;

    fn try_from(row: OutboundRecipientRow) -> Result<Self, Self::Error> {
        Ok(Self {
            kind: OutboundRecipientKind::parse(&row.kind).ok_or_else(|| {
                AppError::Internal(format!("unknown recipient kind {}", row.kind))
            })?,
            address: row.address,
            address_normalized: row.address_normalized,
            display_name: row.display_name,
            position: row.position,
        })
    }
}

pub fn normalize_outbound_from_address(
    from_address: &str,
    configured_domain: &str,
) -> AppResult<OutboundAddress> {
    let configured_domain = configured_domain.trim().to_ascii_lowercase();
    if configured_domain.is_empty() {
        return Err(AppError::Validation(
            "configured outbound domain is required".to_string(),
        ));
    }

    let parsed = parse_route(from_address).map_err(|err| AppError::Validation(err.to_string()))?;
    if parsed.domain != configured_domain {
        return Err(AppError::Validation(format!(
            "from address must use {configured_domain}"
        )));
    }

    Ok(OutboundAddress {
        address_normalized: normalize_email_address(&parsed.address)?,
        address: parsed.address,
    })
}

pub fn normalize_outbound_recipients(
    to: &[String],
    cc: &[String],
    bcc: &[String],
) -> AppResult<Vec<OutboundRecipient>> {
    let mut recipients = Vec::new();
    append_recipients(&mut recipients, OutboundRecipientKind::To, to)?;
    append_recipients(&mut recipients, OutboundRecipientKind::Cc, cc)?;
    append_recipients(&mut recipients, OutboundRecipientKind::Bcc, bcc)?;

    if recipients.is_empty() {
        return Err(AppError::Validation(
            "at least one recipient is required".to_string(),
        ));
    }

    Ok(recipients)
}

pub fn recipient_delivery_addresses(recipients: &[OutboundRecipient]) -> Vec<String> {
    recipients
        .iter()
        .map(|recipient| recipient.address.clone())
        .collect()
}

pub fn build_message_id(domain: &str) -> AppResult<String> {
    let domain = normalize_message_id_domain(domain)?;
    Ok(format!("<{}@{}>", Uuid::new_v4(), domain))
}

pub fn format_rfc2822_date(now: OffsetDateTime) -> AppResult<String> {
    now.format(&Rfc2822)
        .map_err(|err| AppError::Internal(format!("failed to format outbound date: {err}")))
}

pub fn current_rfc2822_date() -> AppResult<String> {
    format_rfc2822_date(OffsetDateTime::now_utc())
}

pub fn reply_subject(subject: &str) -> AppResult<String> {
    let subject = normalize_header_text("subject", subject)?;
    if subject.to_ascii_lowercase().starts_with("re:") {
        Ok(subject)
    } else if subject.is_empty() {
        Ok("Re:".to_string())
    } else {
        Ok(format!("Re: {subject}"))
    }
}

pub fn forward_subject(subject: &str) -> AppResult<String> {
    let subject = normalize_header_text("subject", subject)?;
    let lower = subject.to_ascii_lowercase();
    if lower.starts_with("fwd:") || lower.starts_with("fw:") {
        Ok(subject)
    } else if subject.is_empty() {
        Ok("Fwd:".to_string())
    } else {
        Ok(format!("Fwd: {subject}"))
    }
}

pub fn build_reply_references(
    source_rfc_message_id: Option<&str>,
    source_references: &[String],
) -> AppResult<Vec<String>> {
    let mut references = Vec::new();
    for value in source_references {
        let reference = normalize_message_id_header("reference", value)?;
        if !references.contains(&reference) {
            references.push(reference);
        }
    }

    if let Some(source_rfc_message_id) = source_rfc_message_id {
        let source_rfc_message_id =
            normalize_message_id_header("message id", source_rfc_message_id)?;
        if !references.contains(&source_rfc_message_id) {
            references.push(source_rfc_message_id);
        }
    }

    Ok(references)
}

pub fn build_forward_body(source: &ForwardedMessageSource, note_text: &str) -> String {
    let mut body = String::new();
    let note_text = note_text.trim();
    if !note_text.is_empty() {
        body.push_str(note_text);
        body.push_str("\n\n");
    }

    body.push_str("---------- Forwarded message ---------\n");
    body.push_str("From: ");
    body.push_str(source.from_address.trim());
    body.push('\n');
    body.push_str("Subject: ");
    body.push_str(source.subject.trim());
    body.push_str("\n\n");
    body.push_str(&source.body_text);
    body
}

pub fn build_outbound_mime(message: &OutboundMimeMessage) -> AppResult<Vec<u8>> {
    let from_address = normalize_header_text("from address", &message.from_address)?;
    let to_addresses = normalize_address_headers("to", &message.to_addresses)?;
    let cc_addresses = normalize_address_headers("cc", &message.cc_addresses)?;
    let _bcc_addresses = normalize_address_headers("bcc", &message.bcc_addresses)?;
    if to_addresses.is_empty() && cc_addresses.is_empty() && message.bcc_addresses.is_empty() {
        return Err(AppError::Validation(
            "at least one recipient is required".to_string(),
        ));
    }

    let subject = normalize_header_text("subject", &message.subject)?;
    let message_id = normalize_message_id_header("message id", &message.message_id)?;
    let date = normalize_header_text("date", &message.date)?;
    let in_reply_to = message
        .in_reply_to
        .as_deref()
        .map(|value| normalize_message_id_header("in-reply-to", value))
        .transpose()?;
    let references = message
        .references
        .iter()
        .map(|value| normalize_message_id_header("reference", value))
        .collect::<AppResult<Vec<_>>>()?;
    let reply_to = message
        .reply_to
        .as_deref()
        .map(|value| normalize_header_text("reply-to", value))
        .transpose()?;

    let mut headers = vec![
        format!("From: {from_address}"),
        format!("Message-ID: {message_id}"),
        format!("Date: {date}"),
        format!("Subject: {subject}"),
    ];
    if !to_addresses.is_empty() {
        headers.push(format!("To: {}", to_addresses.join(", ")));
    }
    if !cc_addresses.is_empty() {
        headers.push(format!("Cc: {}", cc_addresses.join(", ")));
    }
    if let Some(reply_to) = reply_to {
        headers.push(format!("Reply-To: {reply_to}"));
    }
    if let Some(in_reply_to) = in_reply_to {
        headers.push(format!("In-Reply-To: {in_reply_to}"));
    }
    if !references.is_empty() {
        headers.push(format!("References: {}", references.join(" ")));
    }
    headers.push("MIME-Version: 1.0".to_string());
    headers.push("Content-Type: text/plain; charset=utf-8".to_string());
    headers.push("Content-Transfer-Encoding: 8bit".to_string());

    Ok(format!(
        "{}\r\n\r\n{}",
        headers.join("\r\n"),
        normalize_body_crlf(&message.body_text)
    )
    .into_bytes())
}

fn append_recipients(
    recipients: &mut Vec<OutboundRecipient>,
    kind: OutboundRecipientKind,
    addresses: &[String],
) -> AppResult<()> {
    for address in addresses {
        let parsed = parse_route(address).map_err(|err| AppError::Validation(err.to_string()))?;
        let address_normalized = normalize_email_address(&parsed.address)?;
        recipients.push(OutboundRecipient {
            kind,
            address: parsed.address,
            address_normalized,
            display_name: String::new(),
            position: recipients.len() as i32,
        });
    }
    Ok(())
}

fn normalize_email_address(address: &str) -> AppResult<String> {
    let parsed = parse_route(address).map_err(|err| AppError::Validation(err.to_string()))?;
    let (local_part, _domain) = parsed
        .address
        .split_once('@')
        .ok_or_else(|| AppError::Validation("address must contain exactly one @".to_string()))?;
    Ok(format!(
        "{}@{}",
        local_part.to_ascii_lowercase(),
        parsed.domain
    ))
}

fn normalize_address_headers(label: &str, addresses: &[String]) -> AppResult<Vec<String>> {
    addresses
        .iter()
        .map(|address| normalize_header_text(label, address))
        .collect()
}

fn normalize_header_text(label: &str, value: &str) -> AppResult<String> {
    let value = value.trim();
    if value.contains('\r') || value.contains('\n') {
        return Err(AppError::Validation(format!(
            "{label} header cannot contain newlines"
        )));
    }
    Ok(value.to_string())
}

fn normalize_message_id_header(label: &str, value: &str) -> AppResult<String> {
    let value = normalize_header_text(label, value)?;
    if value.is_empty() {
        return Err(AppError::Validation(format!("{label} header is required")));
    }
    Ok(value)
}

fn normalize_message_id_domain(domain: &str) -> AppResult<String> {
    let domain = normalize_header_text("message id domain", domain)?.to_ascii_lowercase();
    if domain.is_empty() || domain.contains('@') || domain.split('.').any(str::is_empty) {
        return Err(AppError::Validation(
            "message id domain is invalid".to_string(),
        ));
    }
    Ok(domain)
}

fn normalize_body_crlf(body: &str) -> String {
    body.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\r\n")
}

#[cfg(test)]
mod outbound_types_tests {
    use time::macros::datetime;

    use crate::error::AppError;
    use crate::ports::test_doubles::InMemoryMailSender;

    use super::{
        ComposeMessageRequest, ForwardedMessageSource, InMemoryOutboundService,
        InMemoryReplySource, OutboundMessageStatus, OutboundMimeMessage, OutboundRecipientKind,
        OutboundSendWorker, OutboundService, ReplyMessageRequest, build_forward_body,
        build_message_id, build_outbound_mime, build_reply_references, format_rfc2822_date,
        reply_subject,
    };

    #[test]
    fn outbound_types_validate_from_domain_and_recipients() {
        let request = ComposeMessageRequest {
            from_address: "Contact@Ahara.IO".to_string(),
            to: vec!["Person@Example.COM".to_string()],
            cc: vec!["Team+Ops@Example.COM".to_string()],
            bcc: Vec::new(),
            subject: "hello".to_string(),
            body_text: "plain body".to_string(),
        };

        let validated = request.validate("ahara.io").unwrap();

        assert_eq!(validated.from.address, "Contact@Ahara.IO");
        assert_eq!(validated.from.address_normalized, "contact@ahara.io");
        assert_eq!(validated.recipients.len(), 2);
        assert_eq!(validated.recipients[0].kind, OutboundRecipientKind::To);
        assert_eq!(
            validated.recipients[0].address_normalized,
            "person@example.com"
        );
        assert_eq!(validated.recipients[1].kind, OutboundRecipientKind::Cc);
        assert_eq!(
            validated.recipients[1].address_normalized,
            "team+ops@example.com"
        );
    }

    #[test]
    fn outbound_types_reject_wrong_from_domain_and_missing_recipients() {
        let request = ComposeMessageRequest {
            from_address: "person@example.com".to_string(),
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: "hello".to_string(),
            body_text: "plain body".to_string(),
        };

        assert!(matches!(
            request.validate("ahara.io"),
            Err(AppError::Validation(_))
        ));

        let request = ComposeMessageRequest {
            from_address: "contact@ahara.io".to_string(),
            ..request
        };
        assert!(matches!(
            request.validate("ahara.io"),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn outbound_types_build_text_plain_mime_without_bcc_header() {
        let message = OutboundMimeMessage {
            from_address: "contact@ahara.io".to_string(),
            to_addresses: vec!["person@example.com".to_string()],
            cc_addresses: vec!["team@example.com".to_string()],
            bcc_addresses: vec!["hidden@example.com".to_string()],
            subject: "Quarterly note".to_string(),
            body_text: "hello\nworld".to_string(),
            message_id: "<message-1@ahara.io>".to_string(),
            date: "Thu, 11 Jun 2026 12:34:56 +0000".to_string(),
            in_reply_to: None,
            references: Vec::new(),
            reply_to: None,
        };

        let mime = String::from_utf8(build_outbound_mime(&message).unwrap()).unwrap();

        assert!(mime.contains("From: contact@ahara.io\r\n"));
        assert!(mime.contains("To: person@example.com\r\n"));
        assert!(mime.contains("Cc: team@example.com\r\n"));
        assert!(mime.contains("Subject: Quarterly note\r\n"));
        assert!(mime.contains("Content-Type: text/plain; charset=utf-8\r\n"));
        assert!(mime.contains("\r\n\r\nhello\r\nworld"));
        assert!(!mime.contains("Bcc:"));
        assert!(!mime.contains("text/html"));
    }

    #[test]
    fn outbound_types_build_reply_headers_and_forward_body() {
        let references = build_reply_references(
            Some("<source@sender.test>"),
            &["<thread-root@sender.test>".to_string()],
        )
        .unwrap();
        let body = build_forward_body(
            &ForwardedMessageSource {
                from_address: "sender@example.com".to_string(),
                subject: "Original".to_string(),
                body_text: "source body".to_string(),
            },
            "FYI",
        );
        let message = OutboundMimeMessage {
            from_address: "contact@ahara.io".to_string(),
            to_addresses: vec!["person@example.com".to_string()],
            cc_addresses: Vec::new(),
            bcc_addresses: Vec::new(),
            subject: reply_subject("Original").unwrap(),
            body_text: body,
            message_id: "<reply@ahara.io>".to_string(),
            date: "Thu, 11 Jun 2026 12:34:56 +0000".to_string(),
            in_reply_to: Some("<source@sender.test>".to_string()),
            references,
            reply_to: Some("sender@example.com".to_string()),
        };

        let mime = String::from_utf8(build_outbound_mime(&message).unwrap()).unwrap();

        assert!(mime.contains("Subject: Re: Original\r\n"));
        assert!(mime.contains("Reply-To: sender@example.com\r\n"));
        assert!(mime.contains("In-Reply-To: <source@sender.test>\r\n"));
        assert!(mime.contains("References: <thread-root@sender.test> <source@sender.test>\r\n"));
        assert!(mime.contains("FYI\r\n\r\n---------- Forwarded message ---------"));
    }

    #[test]
    fn outbound_types_reject_header_injection() {
        let message = OutboundMimeMessage {
            from_address: "contact@ahara.io".to_string(),
            to_addresses: vec!["person@example.com".to_string()],
            cc_addresses: Vec::new(),
            bcc_addresses: Vec::new(),
            subject: "hello\r\nBcc: attacker@example.com".to_string(),
            body_text: "body".to_string(),
            message_id: "<message-1@ahara.io>".to_string(),
            date: "Thu, 11 Jun 2026 12:34:56 +0000".to_string(),
            in_reply_to: None,
            references: Vec::new(),
            reply_to: None,
        };

        assert!(matches!(
            build_outbound_mime(&message),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn outbound_types_build_message_ids_and_dates() {
        let message_id = build_message_id("Ahara.IO").unwrap();
        let date = format_rfc2822_date(datetime!(2026-06-11 12:34:56 UTC)).unwrap();

        assert!(message_id.starts_with('<'));
        assert!(message_id.ends_with("@ahara.io>"));
        assert_eq!(date, "Thu, 11 Jun 2026 12:34:56 +0000");
    }

    #[tokio::test]
    async fn outbound_service_compose_enqueues_and_claims_text_plain_work() {
        let service = InMemoryOutboundService::new("ahara.io");
        let queued = service
            .compose_message(ComposeMessageRequest {
                from_address: "contact@ahara.io".to_string(),
                to: vec!["person@example.com".to_string()],
                cc: Vec::new(),
                bcc: vec!["hidden@example.com".to_string()],
                subject: "Plain note".to_string(),
                body_text: "hello".to_string(),
            })
            .await
            .unwrap();

        let detail = service
            .get_outbound_message(&queued.message_id)
            .await
            .unwrap();
        assert_eq!(detail.status, OutboundMessageStatus::Queued);
        assert_eq!(detail.recipients.len(), 2);

        let claimed = service.claim_due_work("worker-1", 25).await.unwrap();
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].attempt_count, 1);
        assert_eq!(
            claimed[0].to_addresses,
            vec!["person@example.com", "hidden@example.com"]
        );
        let raw = String::from_utf8(claimed[0].raw_message.clone()).unwrap();
        assert!(raw.contains("Content-Type: text/plain; charset=utf-8"));
        assert!(!raw.contains("Bcc:"));

        service
            .mark_send_success(&queued.work_id, "ses-provider-1")
            .await
            .unwrap();
        let detail = service
            .get_outbound_message(&queued.message_id)
            .await
            .unwrap();
        assert_eq!(detail.status, OutboundMessageStatus::Sent);
    }

    #[tokio::test]
    async fn outbound_service_lists_outbound_message_summaries() {
        let service = InMemoryOutboundService::new("ahara.io");
        let queued = service
            .compose_message(ComposeMessageRequest {
                from_address: "contact@ahara.io".to_string(),
                to: vec!["person@example.com".to_string()],
                cc: vec!["copy@example.com".to_string()],
                bcc: Vec::new(),
                subject: "Plain note".to_string(),
                body_text: "hello\n\n<script>alert(1)</script> world".to_string(),
            })
            .await
            .unwrap();

        let messages = service.list_outbound_messages().await.unwrap();

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, queued.message_id);
        assert_eq!(messages[0].status, OutboundMessageStatus::Queued);
        assert_eq!(messages[0].from_address, "contact@ahara.io");
        assert_eq!(messages[0].subject, "Plain note");
        assert_eq!(messages[0].snippet, "hello <script>alert(1)</script> world");
        assert_eq!(
            messages[0].primary_recipient.as_deref(),
            Some("person@example.com")
        );
        assert_eq!(messages[0].recipient_count, 2);
    }

    #[tokio::test]
    async fn outbound_service_reply_reuses_source_thread_and_references() {
        let service = InMemoryOutboundService::new("ahara.io");
        let source_id = uuid::Uuid::new_v4();
        let thread_id = uuid::Uuid::new_v4();
        service.seed_reply_source(InMemoryReplySource {
            id: source_id,
            thread_id: Some(thread_id),
            rfc_message_id: Some("<source@example.com>".to_string()),
            reference_ids: vec!["<root@example.com>".to_string()],
            from_address: "sender@example.com".to_string(),
            subject: "Question".to_string(),
        });

        let queued = service
            .reply_to_message(
                &source_id.to_string(),
                ReplyMessageRequest {
                    from_address: "contact@ahara.io".to_string(),
                    to: Vec::new(),
                    cc: Vec::new(),
                    bcc: Vec::new(),
                    body_text: "answer".to_string(),
                },
            )
            .await
            .unwrap();
        let detail = service
            .get_outbound_message(&queued.message_id)
            .await
            .unwrap();

        assert_eq!(detail.thread_id, Some(thread_id.to_string()));
        assert_eq!(detail.subject, "Re: Question");
        assert_eq!(detail.in_reply_to.as_deref(), Some("<source@example.com>"));
        assert_eq!(
            detail.reference_ids,
            vec!["<root@example.com>", "<source@example.com>"]
        );
        assert_eq!(detail.recipients[0].address, "sender@example.com");
    }

    #[tokio::test]
    async fn outbound_service_enforces_suppression_and_rate_limit() {
        let service = InMemoryOutboundService::new("ahara.io");
        service.suppress_address("blocked@example.com");
        let suppressed = service
            .compose_message(ComposeMessageRequest {
                from_address: "contact@ahara.io".to_string(),
                to: vec!["blocked@example.com".to_string()],
                cc: Vec::new(),
                bcc: Vec::new(),
                subject: "Blocked".to_string(),
                body_text: "body".to_string(),
            })
            .await;
        assert!(matches!(suppressed, Err(AppError::Validation(_))));

        for index in 0..super::ENQUEUE_LIMIT_PER_FROM_PER_HOUR {
            service
                .compose_message(ComposeMessageRequest {
                    from_address: "contact@ahara.io".to_string(),
                    to: vec![format!("person-{index}@example.com")],
                    cc: Vec::new(),
                    bcc: Vec::new(),
                    subject: format!("Note {index}"),
                    body_text: "body".to_string(),
                })
                .await
                .unwrap();
        }

        let over_limit = service
            .compose_message(ComposeMessageRequest {
                from_address: "contact@ahara.io".to_string(),
                to: vec!["one-more@example.com".to_string()],
                cc: Vec::new(),
                bcc: Vec::new(),
                subject: "Over".to_string(),
                body_text: "body".to_string(),
            })
            .await;
        assert!(matches!(over_limit, Err(AppError::Validation(_))));
    }

    #[tokio::test]
    async fn outbound_worker_sends_claimed_work_and_marks_success() {
        let service = InMemoryOutboundService::new("ahara.io");
        let sender = InMemoryMailSender::default();
        let queued = service
            .compose_message(ComposeMessageRequest {
                from_address: "contact@ahara.io".to_string(),
                to: vec!["person@example.com".to_string()],
                cc: Vec::new(),
                bcc: Vec::new(),
                subject: "Send it".to_string(),
                body_text: "plain body".to_string(),
            })
            .await
            .unwrap();
        let worker = OutboundSendWorker::new(
            std::sync::Arc::new(service.clone()),
            std::sync::Arc::new(sender.clone()),
            "worker-1",
        );

        let summary = worker.run_once().await.unwrap();

        assert_eq!(summary.claimed, 1);
        assert_eq!(summary.sent, 1);
        assert_eq!(sender.sent().len(), 1);
        assert!(
            String::from_utf8(sender.sent()[0].raw_message.clone())
                .unwrap()
                .contains("Content-Type: text/plain; charset=utf-8")
        );
        let detail = service
            .get_outbound_message(&queued.message_id)
            .await
            .unwrap();
        assert_eq!(detail.status, OutboundMessageStatus::Sent);
    }

    #[tokio::test]
    async fn outbound_worker_retries_transient_send_failures() {
        let service = InMemoryOutboundService::new("ahara.io");
        let sender = InMemoryMailSender::default();
        service
            .compose_message(ComposeMessageRequest {
                from_address: "contact@ahara.io".to_string(),
                to: vec!["person@example.com".to_string()],
                cc: Vec::new(),
                bcc: Vec::new(),
                subject: "Retry it".to_string(),
                body_text: "plain body".to_string(),
            })
            .await
            .unwrap();
        sender.fail_next("ses unavailable");
        let worker = OutboundSendWorker::new(
            std::sync::Arc::new(service),
            std::sync::Arc::new(sender),
            "worker-1",
        );

        let summary = worker.run_once().await.unwrap();

        assert_eq!(summary.claimed, 1);
        assert_eq!(summary.retried, 1);
        assert_eq!(summary.sent, 0);
    }

    #[tokio::test]
    async fn outbound_worker_rechecks_suppressions_before_send() {
        let service = InMemoryOutboundService::new("ahara.io");
        let sender = InMemoryMailSender::default();
        let queued = service
            .compose_message(ComposeMessageRequest {
                from_address: "contact@ahara.io".to_string(),
                to: vec!["person@example.com".to_string()],
                cc: Vec::new(),
                bcc: Vec::new(),
                subject: "Do not send".to_string(),
                body_text: "plain body".to_string(),
            })
            .await
            .unwrap();
        service.suppress_address("person@example.com");
        let worker = OutboundSendWorker::new(
            std::sync::Arc::new(service.clone()),
            std::sync::Arc::new(sender.clone()),
            "worker-1",
        );

        let summary = worker.run_once().await.unwrap();

        assert_eq!(summary.claimed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.suppressed, 1);
        assert!(sender.sent().is_empty());
        let detail = service
            .get_outbound_message(&queued.message_id)
            .await
            .unwrap();
        assert_eq!(detail.status, OutboundMessageStatus::Failed);
    }
}
