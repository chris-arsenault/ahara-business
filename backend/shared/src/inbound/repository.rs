use async_trait::async_trait;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::inbound::ses_event::InboundReceipt;
use crate::inbound::threading::{build_thread_seed, participants_json};
use crate::inbound::types::{
    AuthResult, InboundAuthResults, InboundMailbox, InboundMessageStatus, InboundRoutingMatch,
    InboundSecurityRecord, ParsedInboundMessage, PersistedInboundMessage, RejectedInboundAudit,
};
use crate::mail_security::{SecurityDisposition, SecurityReason};

#[derive(Debug, Clone)]
pub struct PersistInboundMessageRequest {
    pub receipt: InboundReceipt,
    pub parsed: ParsedInboundMessage,
    pub auth: InboundAuthResults,
    pub security: InboundSecurityRecord,
    pub routing: InboundRoutingMatch,
}

#[derive(Debug, Clone)]
pub struct PersistRejectedInboundRequest {
    pub audit: RejectedInboundAudit,
    pub auth: InboundAuthResults,
}

#[derive(Debug, Clone)]
pub struct RejectedAuditRequest {
    pub ses_message_id: String,
    pub s3_raw_key: Option<String>,
    pub envelope_recipients: Vec<String>,
    pub from: Option<InboundMailbox>,
    pub security: InboundSecurityRecord,
    pub rejection_reason: String,
    pub size_bytes: Option<i64>,
}

#[async_trait]
pub trait InboundRepository: Send + Sync {
    async fn persist_inbound(
        &self,
        request: PersistInboundMessageRequest,
    ) -> AppResult<PersistedInboundMessage>;

    async fn persist_rejected_inbound(
        &self,
        request: PersistRejectedInboundRequest,
    ) -> AppResult<PersistedInboundMessage>;

    async fn recent_raw_mail_bytes(&self, window_seconds: i64) -> AppResult<i64>;
}

#[derive(Debug, Clone)]
pub struct PgInboundRepository {
    pool: DbPool,
}

impl PgInboundRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl InboundRepository for PgInboundRepository {
    async fn persist_inbound(
        &self,
        request: PersistInboundMessageRequest,
    ) -> AppResult<PersistedInboundMessage> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        if let Some(existing) = find_existing(
            &mut tx,
            &request.receipt.ses_message_id,
            &request.receipt.raw_mail.key,
        )
        .await?
        {
            tx.commit()
                .await
                .map_err(|err| AppError::Database(err.to_string()))?;
            return Ok(existing);
        }

        let thread_id = upsert_thread(&mut tx, &request.parsed).await?;
        let contact_id = find_contact_id(&mut tx, &request.parsed.from).await?;
        let status = InboundMessageStatus::from_security_disposition(request.security.disposition);
        let message_id =
            insert_message(&mut tx, &request, Some(thread_id), contact_id, status).await?;
        insert_recipients(&mut tx, message_id, &request.parsed).await?;
        insert_attachments(&mut tx, message_id, &request.parsed).await?;

        tx.commit()
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;

        Ok(PersistedInboundMessage {
            id: message_id,
            status,
            security_disposition: request.security.disposition,
            idempotent: false,
        })
    }

    async fn persist_rejected_inbound(
        &self,
        request: PersistRejectedInboundRequest,
    ) -> AppResult<PersistedInboundMessage> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        if let Some(existing) = find_existing(
            &mut tx,
            &request.audit.ses_message_id,
            request.audit.s3_raw_key.as_deref().unwrap_or_default(),
        )
        .await?
        {
            tx.commit()
                .await
                .map_err(|err| AppError::Database(err.to_string()))?;
            return Ok(existing);
        }

        let message_id = insert_rejected_audit(&mut tx, &request).await?;
        insert_envelope_recipients(&mut tx, message_id, &request.audit.envelope_recipients).await?;
        tx.commit()
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;

        Ok(PersistedInboundMessage {
            id: message_id,
            status: InboundMessageStatus::Rejected,
            security_disposition: SecurityDisposition::Rejected,
            idempotent: false,
        })
    }

    async fn recent_raw_mail_bytes(&self, window_seconds: i64) -> AppResult<i64> {
        let window_seconds = window_seconds.max(0);
        let total: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(size_bytes), 0)::BIGINT
             FROM messages
             WHERE direction = 'inbound'
               AND size_bytes IS NOT NULL
               AND received_at >= now() - ($1::DOUBLE PRECISION * interval '1 second')",
        )
        .bind(window_seconds as f64)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        Ok(total)
    }
}

async fn find_existing(
    tx: &mut Transaction<'_, Postgres>,
    ses_message_id: &str,
    s3_raw_key: &str,
) -> AppResult<Option<PersistedInboundMessage>> {
    let row: Option<MessageSummaryRow> = sqlx::query_as(
        "SELECT id, status, security_disposition
         FROM messages
         WHERE ses_message_id = $1
            OR ($2 <> '' AND s3_raw_key = $2)
         ORDER BY created_at
         LIMIT 1",
    )
    .bind(ses_message_id)
    .bind(s3_raw_key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|err| AppError::Database(err.to_string()))?;

    row.map(|row| row.into_summary(true)).transpose()
}

async fn upsert_thread(
    tx: &mut Transaction<'_, Postgres>,
    parsed: &ParsedInboundMessage,
) -> AppResult<Uuid> {
    let seed = build_thread_seed(parsed);
    let candidates = seed
        .in_reply_to
        .clone()
        .into_iter()
        .chain(seed.reference_ids.clone())
        .collect::<Vec<_>>();
    let existing: Option<ThreadRow> = if candidates.is_empty() {
        None
    } else {
        sqlx::query_as(
            "SELECT thread_id AS id
             FROM messages
             WHERE rfc_message_id = ANY($1)
               AND thread_id IS NOT NULL
             ORDER BY created_at DESC
             LIMIT 1",
        )
        .bind(&candidates)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?
    };

    let participants = participants_json(&seed.participants);
    let activity_epoch = seed.activity_epoch.map(|epoch| epoch as f64);
    if let Some(existing) = existing {
        let merged = merge_thread_participants(tx, existing.id, &seed.participants).await?;
        sqlx::query(
            "UPDATE threads
             SET participants = $2,
                 last_activity_at = GREATEST(last_activity_at, COALESCE(to_timestamp($3::double precision), now())),
                 message_count = message_count + 1,
                 updated_at = now()
             WHERE id = $1",
        )
        .bind(existing.id)
        .bind(merged)
        .bind(activity_epoch)
        .execute(&mut **tx)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;
        return Ok(existing.id);
    }

    let row: ThreadRow = sqlx::query_as(
        "INSERT INTO threads (normalized_subject, participants, last_activity_at, message_count)
         VALUES ($1, $2, COALESCE(to_timestamp($3::double precision), now()), 1)
         RETURNING id",
    )
    .bind(&seed.normalized_subject)
    .bind(participants)
    .bind(activity_epoch)
    .fetch_one(&mut **tx)
    .await
    .map_err(|err| AppError::Database(err.to_string()))?;
    Ok(row.id)
}

async fn merge_thread_participants(
    tx: &mut Transaction<'_, Postgres>,
    thread_id: Uuid,
    new_participants: &[String],
) -> AppResult<Value> {
    let row: ThreadParticipantsRow = sqlx::query_as(
        "SELECT participants
         FROM threads
         WHERE id = $1",
    )
    .bind(thread_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|err| AppError::Database(err.to_string()))?;

    let mut participants = row
        .participants
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| value.as_str().map(ToString::to_string))
        .collect::<Vec<_>>();
    for participant in new_participants {
        if !participants.contains(participant) {
            participants.push(participant.clone());
        }
    }
    Ok(participants_json(&participants))
}

async fn find_contact_id(
    tx: &mut Transaction<'_, Postgres>,
    sender: &InboundMailbox,
) -> AppResult<Option<Uuid>> {
    let row: Option<ContactIdRow> = sqlx::query_as(
        "SELECT id
         FROM contacts
         WHERE primary_address_normalized = $1",
    )
    .bind(&sender.address_normalized)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|err| AppError::Database(err.to_string()))?;
    Ok(row.map(|row| row.id))
}

async fn insert_message(
    tx: &mut Transaction<'_, Postgres>,
    request: &PersistInboundMessageRequest,
    thread_id: Option<Uuid>,
    contact_id: Option<Uuid>,
    status: InboundMessageStatus,
) -> AppResult<Uuid> {
    let message_date_epoch = request.parsed.message_date_epoch.map(|epoch| epoch as f64);
    let row: MessageIdRow = sqlx::query_as(
        "INSERT INTO messages (
             direction, ses_message_id, rfc_message_id, in_reply_to, reference_ids, thread_id,
             from_address, from_address_normalized, from_display_name, subject, message_date,
             matched_domain_id, matched_address_id, matched_local_part, plus_tag, body_text,
             s3_raw_key, spf_result, dkim_result, dmarc_result, auth_verdict, spam_result,
             virus_result, security_disposition, security_reason, contact_id, status,
             has_attachments, attachment_count, size_bytes, received_at
         )
         VALUES (
             'inbound', $1, $2, $3, $4, $5,
             $6, $7, $8, $9, to_timestamp($10::double precision),
             $11, $12, $13, $14, $15,
             $16, $17, $18, $19, $20, $21,
             $22, $23, $24, $25, $26,
             $27, $28, $29, now()
         )
         RETURNING id",
    )
    .bind(&request.receipt.ses_message_id)
    .bind(&request.parsed.rfc_message_id)
    .bind(&request.parsed.in_reply_to)
    .bind(&request.parsed.reference_ids)
    .bind(thread_id)
    .bind(&request.parsed.from.address)
    .bind(&request.parsed.from.address_normalized)
    .bind(&request.parsed.from.display_name)
    .bind(&request.parsed.subject)
    .bind(message_date_epoch)
    .bind(request.routing.domain_id)
    .bind(request.routing.address_id)
    .bind(&request.routing.matched_local_part)
    .bind(&request.routing.plus_tag)
    .bind(&request.parsed.body_text)
    .bind(&request.receipt.raw_mail.key)
    .bind(auth_db(request.auth.spf))
    .bind(auth_db(request.auth.dkim))
    .bind(auth_db(request.auth.dmarc))
    .bind(auth_db(request.auth.auth_verdict))
    .bind(&request.security.spam_result)
    .bind(&request.security.virus_result)
    .bind(request.security.disposition.as_db_value())
    .bind(request.security.reason.as_db_value())
    .bind(contact_id)
    .bind(status.as_db_value())
    .bind(request.parsed.has_attachments())
    .bind(request.parsed.attachments.len() as i32)
    .bind(request.parsed.size_bytes)
    .fetch_one(&mut **tx)
    .await
    .map_err(|err| AppError::Database(err.to_string()))?;
    Ok(row.id)
}

async fn insert_recipients(
    tx: &mut Transaction<'_, Postgres>,
    message_id: Uuid,
    parsed: &ParsedInboundMessage,
) -> AppResult<()> {
    for recipient in &parsed.recipients {
        sqlx::query(
            "INSERT INTO recipients (
                 message_id, kind, address, address_normalized, display_name, position
             )
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(message_id)
        .bind(recipient.kind.as_db_value())
        .bind(&recipient.mailbox.address)
        .bind(&recipient.mailbox.address_normalized)
        .bind(&recipient.mailbox.display_name)
        .bind(recipient.position)
        .execute(&mut **tx)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;
    }
    Ok(())
}

async fn insert_attachments(
    tx: &mut Transaction<'_, Postgres>,
    message_id: Uuid,
    parsed: &ParsedInboundMessage,
) -> AppResult<()> {
    for attachment in &parsed.attachments {
        sqlx::query(
            "INSERT INTO attachment_refs (
                 message_id, position, filename, content_type, size_bytes, content_id
             )
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(message_id)
        .bind(attachment.position)
        .bind(&attachment.filename)
        .bind(&attachment.content_type)
        .bind(attachment.size_bytes)
        .bind(&attachment.content_id)
        .execute(&mut **tx)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;
    }
    Ok(())
}

async fn insert_rejected_audit(
    tx: &mut Transaction<'_, Postgres>,
    request: &PersistRejectedInboundRequest,
) -> AppResult<Uuid> {
    let row: MessageIdRow = sqlx::query_as(
        "INSERT INTO messages (
             direction, ses_message_id, from_address, from_address_normalized, from_display_name,
             body_text, s3_raw_key, spf_result, dkim_result, dmarc_result, auth_verdict,
             spam_result, virus_result, security_disposition, security_reason, status,
             has_attachments, attachment_count, size_bytes, received_at
         )
         VALUES (
             'inbound', $1, $2, $3, $4,
             '', $5, $6, $7, $8, $9,
             $10, $11, 'rejected', $12, 'rejected',
             false, 0, $13, now()
         )
         RETURNING id",
    )
    .bind(&request.audit.ses_message_id)
    .bind(&request.audit.from.address)
    .bind(&request.audit.from.address_normalized)
    .bind(&request.audit.from.display_name)
    .bind(&request.audit.s3_raw_key)
    .bind(auth_db(request.auth.spf))
    .bind(auth_db(request.auth.dkim))
    .bind(auth_db(request.auth.dmarc))
    .bind(auth_db(request.auth.auth_verdict))
    .bind(&request.audit.security.spam_result)
    .bind(&request.audit.security.virus_result)
    .bind(&request.audit.rejection_reason)
    .bind(request.audit.size_bytes)
    .fetch_one(&mut **tx)
    .await
    .map_err(|err| AppError::Database(err.to_string()))?;
    Ok(row.id)
}

async fn insert_envelope_recipients(
    tx: &mut Transaction<'_, Postgres>,
    message_id: Uuid,
    recipients: &[String],
) -> AppResult<()> {
    for (position, recipient) in recipients
        .iter()
        .map(|recipient| recipient.trim())
        .filter(|recipient| !recipient.is_empty())
        .enumerate()
    {
        sqlx::query(
            "INSERT INTO recipients (
                 message_id, kind, address, address_normalized, display_name, position
             )
             VALUES ($1, 'to', $2, $3, '', $4)",
        )
        .bind(message_id)
        .bind(recipient)
        .bind(recipient.to_ascii_lowercase())
        .bind(position as i32)
        .execute(&mut **tx)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;
    }
    Ok(())
}

fn auth_db(value: Option<AuthResult>) -> Option<&'static str> {
    value.map(AuthResult::as_db_value)
}

#[derive(Debug, sqlx::FromRow)]
struct MessageSummaryRow {
    id: Uuid,
    status: String,
    security_disposition: String,
}

impl MessageSummaryRow {
    fn into_summary(self, idempotent: bool) -> AppResult<PersistedInboundMessage> {
        Ok(PersistedInboundMessage {
            id: self.id,
            status: status_from_db(&self.status)?,
            security_disposition: disposition_from_db(&self.security_disposition)?,
            idempotent,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct MessageIdRow {
    id: Uuid,
}

#[derive(Debug, sqlx::FromRow)]
struct ThreadRow {
    id: Uuid,
}

#[derive(Debug, sqlx::FromRow)]
struct ThreadParticipantsRow {
    participants: Value,
}

#[derive(Debug, sqlx::FromRow)]
struct ContactIdRow {
    id: Uuid,
}

fn status_from_db(status: &str) -> AppResult<InboundMessageStatus> {
    match status {
        "received" => Ok(InboundMessageStatus::Received),
        "quarantined" => Ok(InboundMessageStatus::Quarantined),
        "rejected" => Ok(InboundMessageStatus::Rejected),
        _ => Err(AppError::Internal(format!(
            "unknown inbound status {status}"
        ))),
    }
}

fn disposition_from_db(disposition: &str) -> AppResult<SecurityDisposition> {
    match disposition {
        "accepted" => Ok(SecurityDisposition::Accepted),
        "quarantined" => Ok(SecurityDisposition::Quarantined),
        "rejected" => Ok(SecurityDisposition::Rejected),
        _ => Err(AppError::Internal(format!(
            "unknown security disposition {disposition}"
        ))),
    }
}

pub fn rejected_audit(request: RejectedAuditRequest) -> RejectedInboundAudit {
    RejectedInboundAudit {
        ses_message_id: request.ses_message_id,
        s3_raw_key: request.s3_raw_key,
        envelope_recipients: request.envelope_recipients,
        from: request.from.unwrap_or_else(InboundMailbox::unknown),
        status: InboundMessageStatus::Rejected,
        security: request.security,
        rejection_reason: request.rejection_reason,
        size_bytes: request.size_bytes,
    }
}

pub fn rejected_security(reason: SecurityReason) -> InboundSecurityRecord {
    InboundSecurityRecord {
        disposition: SecurityDisposition::Rejected,
        reason,
        spam_result: None,
        virus_result: None,
    }
}
