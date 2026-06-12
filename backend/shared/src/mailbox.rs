use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};

pub const DEFAULT_MAILBOX_LIMIT: u32 = 50;
pub const MAX_MAILBOX_LIMIT: u32 = 100;
pub const DEFAULT_SNIPPET_CHARS: usize = 180;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailboxMessageSummary {
    pub id: String,
    pub thread_id: Option<String>,
    pub from_address: String,
    pub from_display_name: String,
    pub subject: String,
    pub snippet: String,
    pub received_at: Option<String>,
    pub is_read: bool,
    pub has_attachments: bool,
    pub attachment_count: i32,
    pub contact_id: Option<String>,
    pub auth_verdict: Option<MailboxAuthResult>,
    pub spam_result: Option<MailboxScanResult>,
    pub virus_result: Option<MailboxScanResult>,
    pub security_disposition: MailboxSecurityDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailboxMessageDetail {
    pub id: String,
    pub thread_id: Option<String>,
    pub rfc_message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub reference_ids: Vec<String>,
    pub from_address: String,
    pub from_display_name: String,
    pub subject: String,
    pub message_date: Option<String>,
    pub received_at: Option<String>,
    pub body_text: String,
    pub recipients: Vec<MailboxRecipient>,
    pub attachments: Vec<MailboxAttachment>,
    pub is_read: bool,
    pub contact_id: Option<String>,
    pub spf_result: Option<MailboxAuthResult>,
    pub dkim_result: Option<MailboxAuthResult>,
    pub dmarc_result: Option<MailboxAuthResult>,
    pub auth_verdict: Option<MailboxAuthResult>,
    pub spam_result: Option<MailboxScanResult>,
    pub virus_result: Option<MailboxScanResult>,
    pub security_disposition: MailboxSecurityDisposition,
    pub security_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailboxThreadDetail {
    pub thread_id: String,
    pub normalized_subject: String,
    pub message_count: i32,
    pub last_activity_at: Option<String>,
    pub messages: Vec<MailboxMessageDetail>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailboxRecipient {
    pub kind: MailboxRecipientKind,
    pub address: String,
    pub address_normalized: String,
    pub display_name: String,
    pub position: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MailboxRecipientKind {
    To,
    Cc,
    Bcc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailboxAttachment {
    pub id: String,
    pub position: i32,
    pub filename: String,
    pub display_filename: String,
    pub content_type: String,
    pub size_bytes: Option<i64>,
    pub content_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailboxQuery {
    pub limit: Option<u32>,
    pub unread_only: Option<bool>,
    pub before_received_at: Option<String>,
}

impl MailboxQuery {
    pub fn validated_limit(&self) -> AppResult<u32> {
        validate_limit(self.limit)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailboxSearchQuery {
    pub q: String,
    pub limit: Option<u32>,
}

impl MailboxSearchQuery {
    pub fn validated(&self) -> AppResult<ValidatedMailboxSearchQuery> {
        let q = self.q.trim();
        if q.is_empty() {
            return Err(AppError::Validation("search query is required".to_string()));
        }

        Ok(ValidatedMailboxSearchQuery {
            q: q.to_string(),
            limit: validate_limit(self.limit)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedMailboxSearchQuery {
    pub q: String,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateMessageStateRequest {
    pub is_read: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkMessageContactRequest {
    pub contact_id: Option<String>,
}

impl LinkMessageContactRequest {
    pub fn validated_contact_id(&self) -> AppResult<Option<Uuid>> {
        self.contact_id
            .as_deref()
            .map(str::trim)
            .filter(|contact_id| !contact_id.is_empty())
            .map(|contact_id| {
                Uuid::parse_str(contact_id)
                    .map_err(|_| AppError::Validation("contact id must be a UUID".to_string()))
            })
            .transpose()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MailboxAuthResult {
    Pass,
    Fail,
    Neutral,
    Softfail,
    Temperror,
    Permerror,
    None,
}

impl MailboxAuthResult {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pass" => Some(Self::Pass),
            "fail" => Some(Self::Fail),
            "neutral" => Some(Self::Neutral),
            "softfail" => Some(Self::Softfail),
            "temperror" => Some(Self::Temperror),
            "permerror" => Some(Self::Permerror),
            "none" => Some(Self::None),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MailboxScanResult {
    Pass,
    Fail,
    Gray,
    ProcessingFailed,
}

impl MailboxScanResult {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pass" => Some(Self::Pass),
            "fail" => Some(Self::Fail),
            "gray" => Some(Self::Gray),
            "processing_failed" | "processingfailed" => Some(Self::ProcessingFailed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MailboxSecurityDisposition {
    Accepted,
    Quarantined,
    Rejected,
}

impl MailboxSecurityDisposition {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "accepted" => Some(Self::Accepted),
            "quarantined" => Some(Self::Quarantined),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }

    pub fn as_db_value(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Quarantined => "quarantined",
            Self::Rejected => "rejected",
        }
    }
}

#[async_trait]
pub trait MailboxService: Send + Sync {
    async fn list_messages(&self, query: MailboxQuery) -> AppResult<Vec<MailboxMessageSummary>>;
    async fn get_message(&self, message_id: &str) -> AppResult<MailboxMessageDetail>;
    async fn get_thread(&self, thread_id: &str) -> AppResult<MailboxThreadDetail>;
    async fn search_messages(
        &self,
        query: MailboxSearchQuery,
    ) -> AppResult<Vec<MailboxMessageSummary>>;
    async fn update_message_state(
        &self,
        message_id: &str,
        request: UpdateMessageStateRequest,
    ) -> AppResult<MailboxMessageSummary>;
    async fn link_message_contact(
        &self,
        message_id: &str,
        request: LinkMessageContactRequest,
    ) -> AppResult<MailboxMessageSummary>;
}

#[derive(Debug, Clone)]
pub struct PgMailboxService {
    pool: DbPool,
}

impl PgMailboxService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MailboxService for PgMailboxService {
    async fn list_messages(&self, query: MailboxQuery) -> AppResult<Vec<MailboxMessageSummary>> {
        let limit = i64::from(query.validated_limit()?);
        let unread_only = query.unread_only.unwrap_or(false);
        let rows: Vec<MessageSummaryRow> = sqlx::query_as(
            "SELECT id,
                    thread_id,
                    from_address,
                    from_display_name,
                    subject,
                    body_text,
                    received_at::text AS received_at,
                    is_read,
                    has_attachments,
                    attachment_count,
                    contact_id,
                    auth_verdict,
                    spam_result,
                    virus_result,
                    security_disposition
             FROM messages
             WHERE direction = 'inbound'
               AND security_disposition = 'accepted'
               AND status = 'received'
               AND ($1::boolean = false OR is_read = false)
               AND ($2::text IS NULL OR received_at < $2::timestamptz)
             ORDER BY received_at DESC NULLS LAST, created_at DESC, id DESC
             LIMIT $3",
        )
        .bind(unread_only)
        .bind(query.before_received_at)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn get_message(&self, message_id: &str) -> AppResult<MailboxMessageDetail> {
        let message_id = parse_uuid(message_id, "message id")?;
        self.fetch_message_detail(message_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("message {message_id}")))
    }

    async fn get_thread(&self, thread_id: &str) -> AppResult<MailboxThreadDetail> {
        let thread_id = parse_uuid(thread_id, "thread id")?;
        let row: Option<ThreadRow> = sqlx::query_as(
            "SELECT id,
                    normalized_subject,
                    last_activity_at::text AS last_activity_at
             FROM threads
             WHERE id = $1",
        )
        .bind(thread_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;
        let row = row.ok_or_else(|| AppError::NotFound(format!("thread {thread_id}")))?;

        let detail_rows: Vec<MessageDetailRow> = sqlx::query_as(
            "SELECT id,
                    thread_id,
                    rfc_message_id,
                    in_reply_to,
                    reference_ids,
                    from_address,
                    from_display_name,
                    subject,
                    message_date::text AS message_date,
                    received_at::text AS received_at,
                    body_text,
                    is_read,
                    contact_id,
                    spf_result,
                    dkim_result,
                    dmarc_result,
                    auth_verdict,
                    spam_result,
                    virus_result,
                    security_disposition,
                    security_reason
             FROM messages
             WHERE thread_id = $1
               AND direction = 'inbound'
               AND security_disposition = 'accepted'
               AND status = 'received'
             ORDER BY received_at ASC NULLS LAST, created_at ASC, id ASC",
        )
        .bind(thread_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        let mut messages = Vec::with_capacity(detail_rows.len());
        for detail_row in detail_rows {
            messages.push(self.detail_from_row(detail_row).await?);
        }
        if messages.is_empty() {
            return Err(AppError::NotFound(format!("thread {thread_id}")));
        }

        Ok(MailboxThreadDetail {
            thread_id: row.id.to_string(),
            normalized_subject: row.normalized_subject,
            message_count: messages.len() as i32,
            last_activity_at: row.last_activity_at,
            messages,
        })
    }

    async fn search_messages(
        &self,
        query: MailboxSearchQuery,
    ) -> AppResult<Vec<MailboxMessageSummary>> {
        let query = query.validated()?;
        let needle = query.q.to_ascii_lowercase();
        let rows: Vec<MessageSummaryRow> = sqlx::query_as(
            "SELECT id,
                    thread_id,
                    from_address,
                    from_display_name,
                    subject,
                    body_text,
                    received_at::text AS received_at,
                    is_read,
                    has_attachments,
                    attachment_count,
                    contact_id,
                    auth_verdict,
                    spam_result,
                    virus_result,
                    security_disposition
             FROM messages
             WHERE direction = 'inbound'
               AND security_disposition = 'accepted'
               AND status = 'received'
               AND (
                    position($1 in lower(subject)) > 0
                 OR position($1 in lower(from_address)) > 0
                 OR position($1 in lower(from_display_name)) > 0
                 OR position($1 in lower(body_text)) > 0
               )
             ORDER BY received_at DESC NULLS LAST, created_at DESC, id DESC
             LIMIT $2",
        )
        .bind(needle)
        .bind(i64::from(query.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn update_message_state(
        &self,
        message_id: &str,
        request: UpdateMessageStateRequest,
    ) -> AppResult<MailboxMessageSummary> {
        let message_id = parse_uuid(message_id, "message id")?;
        let row: Option<MessageSummaryRow> = sqlx::query_as(
            "UPDATE messages
             SET is_read = $2,
                 updated_at = now()
             WHERE id = $1
               AND direction = 'inbound'
               AND security_disposition = 'accepted'
               AND status = 'received'
             RETURNING id,
                       thread_id,
                       from_address,
                       from_display_name,
                       subject,
                       body_text,
                       received_at::text AS received_at,
                       is_read,
                       has_attachments,
                       attachment_count,
                       contact_id,
                       auth_verdict,
                       spam_result,
                       virus_result,
                       security_disposition",
        )
        .bind(message_id)
        .bind(request.is_read)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        row.ok_or_else(|| AppError::NotFound(format!("message {message_id}")))?
            .try_into()
    }

    async fn link_message_contact(
        &self,
        message_id: &str,
        request: LinkMessageContactRequest,
    ) -> AppResult<MailboxMessageSummary> {
        let message_id = parse_uuid(message_id, "message id")?;
        let contact_id = request.validated_contact_id()?;
        if let Some(contact_id) = contact_id {
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM contacts WHERE id = $1)")
                    .bind(contact_id)
                    .fetch_one(&self.pool)
                    .await
                    .map_err(|err| AppError::Database(err.to_string()))?;
            if !exists {
                return Err(AppError::NotFound(format!("contact {contact_id}")));
            }
        }

        let row: Option<MessageSummaryRow> = sqlx::query_as(
            "UPDATE messages
             SET contact_id = $2,
                 updated_at = now()
             WHERE id = $1
               AND direction = 'inbound'
               AND security_disposition = 'accepted'
               AND status = 'received'
             RETURNING id,
                       thread_id,
                       from_address,
                       from_display_name,
                       subject,
                       body_text,
                       received_at::text AS received_at,
                       is_read,
                       has_attachments,
                       attachment_count,
                       contact_id,
                       auth_verdict,
                       spam_result,
                       virus_result,
                       security_disposition",
        )
        .bind(message_id)
        .bind(contact_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        row.ok_or_else(|| AppError::NotFound(format!("message {message_id}")))?
            .try_into()
    }
}

impl PgMailboxService {
    async fn fetch_message_detail(
        &self,
        message_id: Uuid,
    ) -> AppResult<Option<MailboxMessageDetail>> {
        let row: Option<MessageDetailRow> = sqlx::query_as(
            "SELECT id,
                    thread_id,
                    rfc_message_id,
                    in_reply_to,
                    reference_ids,
                    from_address,
                    from_display_name,
                    subject,
                    message_date::text AS message_date,
                    received_at::text AS received_at,
                    body_text,
                    is_read,
                    contact_id,
                    spf_result,
                    dkim_result,
                    dmarc_result,
                    auth_verdict,
                    spam_result,
                    virus_result,
                    security_disposition,
                    security_reason
             FROM messages
             WHERE id = $1
               AND direction = 'inbound'
               AND security_disposition = 'accepted'
               AND status = 'received'",
        )
        .bind(message_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        match row {
            Some(row) => Ok(Some(self.detail_from_row(row).await?)),
            None => Ok(None),
        }
    }

    async fn detail_from_row(&self, row: MessageDetailRow) -> AppResult<MailboxMessageDetail> {
        let recipients: Vec<RecipientRow> = sqlx::query_as(
            "SELECT kind,
                    address,
                    address_normalized,
                    display_name,
                    position
             FROM recipients
             WHERE message_id = $1
             ORDER BY position ASC, id ASC",
        )
        .bind(row.id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        let attachments: Vec<AttachmentRow> = sqlx::query_as(
            "SELECT id,
                    position,
                    filename,
                    content_type,
                    size_bytes,
                    content_id
             FROM attachment_refs
             WHERE message_id = $1
             ORDER BY position ASC, id ASC",
        )
        .bind(row.id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        row.into_detail(recipients, attachments)
    }
}

#[derive(Debug, sqlx::FromRow)]
struct MessageSummaryRow {
    id: Uuid,
    thread_id: Option<Uuid>,
    from_address: String,
    from_display_name: String,
    subject: String,
    body_text: String,
    received_at: Option<String>,
    is_read: bool,
    has_attachments: bool,
    attachment_count: i32,
    contact_id: Option<Uuid>,
    auth_verdict: Option<String>,
    spam_result: Option<String>,
    virus_result: Option<String>,
    security_disposition: String,
}

impl TryFrom<MessageSummaryRow> for MailboxMessageSummary {
    type Error = AppError;

    fn try_from(value: MessageSummaryRow) -> AppResult<Self> {
        Ok(Self {
            id: value.id.to_string(),
            thread_id: value.thread_id.map(|id| id.to_string()),
            from_address: value.from_address,
            from_display_name: value.from_display_name,
            subject: value.subject,
            snippet: body_snippet(&value.body_text, DEFAULT_SNIPPET_CHARS),
            received_at: value.received_at,
            is_read: value.is_read,
            has_attachments: value.has_attachments,
            attachment_count: value.attachment_count,
            contact_id: value.contact_id.map(|id| id.to_string()),
            auth_verdict: parse_optional_auth_result(value.auth_verdict.as_deref())?,
            spam_result: parse_optional_scan_result(value.spam_result.as_deref())?,
            virus_result: parse_optional_scan_result(value.virus_result.as_deref())?,
            security_disposition: parse_security_disposition(&value.security_disposition)?,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct MessageDetailRow {
    id: Uuid,
    thread_id: Option<Uuid>,
    rfc_message_id: Option<String>,
    in_reply_to: Option<String>,
    reference_ids: Vec<String>,
    from_address: String,
    from_display_name: String,
    subject: String,
    message_date: Option<String>,
    received_at: Option<String>,
    body_text: String,
    is_read: bool,
    contact_id: Option<Uuid>,
    spf_result: Option<String>,
    dkim_result: Option<String>,
    dmarc_result: Option<String>,
    auth_verdict: Option<String>,
    spam_result: Option<String>,
    virus_result: Option<String>,
    security_disposition: String,
    security_reason: Option<String>,
}

impl MessageDetailRow {
    fn into_detail(
        self,
        recipients: Vec<RecipientRow>,
        attachments: Vec<AttachmentRow>,
    ) -> AppResult<MailboxMessageDetail> {
        Ok(MailboxMessageDetail {
            id: self.id.to_string(),
            thread_id: self.thread_id.map(|id| id.to_string()),
            rfc_message_id: self.rfc_message_id,
            in_reply_to: self.in_reply_to,
            reference_ids: self.reference_ids,
            from_address: self.from_address,
            from_display_name: self.from_display_name,
            subject: self.subject,
            message_date: self.message_date,
            received_at: self.received_at,
            body_text: self.body_text,
            recipients: recipients
                .into_iter()
                .map(TryInto::try_into)
                .collect::<AppResult<Vec<_>>>()?,
            attachments: attachments.into_iter().map(Into::into).collect(),
            is_read: self.is_read,
            contact_id: self.contact_id.map(|id| id.to_string()),
            spf_result: parse_optional_auth_result(self.spf_result.as_deref())?,
            dkim_result: parse_optional_auth_result(self.dkim_result.as_deref())?,
            dmarc_result: parse_optional_auth_result(self.dmarc_result.as_deref())?,
            auth_verdict: parse_optional_auth_result(self.auth_verdict.as_deref())?,
            spam_result: parse_optional_scan_result(self.spam_result.as_deref())?,
            virus_result: parse_optional_scan_result(self.virus_result.as_deref())?,
            security_disposition: parse_security_disposition(&self.security_disposition)?,
            security_reason: self.security_reason,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct RecipientRow {
    kind: String,
    address: String,
    address_normalized: String,
    display_name: String,
    position: i32,
}

impl TryFrom<RecipientRow> for MailboxRecipient {
    type Error = AppError;

    fn try_from(value: RecipientRow) -> AppResult<Self> {
        Ok(Self {
            kind: parse_recipient_kind(&value.kind)?,
            address: value.address,
            address_normalized: value.address_normalized,
            display_name: value.display_name,
            position: value.position,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct AttachmentRow {
    id: Uuid,
    position: i32,
    filename: String,
    content_type: String,
    size_bytes: Option<i64>,
    content_id: Option<String>,
}

impl From<AttachmentRow> for MailboxAttachment {
    fn from(value: AttachmentRow) -> Self {
        Self {
            id: value.id.to_string(),
            position: value.position,
            display_filename: sanitize_attachment_filename(&value.filename),
            filename: value.filename,
            content_type: value.content_type,
            size_bytes: value.size_bytes,
            content_id: value.content_id,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct ThreadRow {
    id: Uuid,
    normalized_subject: String,
    last_activity_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InMemoryMailboxMessage {
    pub direction: String,
    pub status: String,
    pub normalized_subject: String,
    pub last_activity_at: Option<String>,
    pub detail: MailboxMessageDetail,
}

impl InMemoryMailboxMessage {
    pub fn accepted(detail: MailboxMessageDetail) -> Self {
        Self {
            normalized_subject: detail.subject.to_ascii_lowercase(),
            last_activity_at: detail.received_at.clone(),
            direction: "inbound".to_string(),
            status: "received".to_string(),
            detail,
        }
    }

    fn is_normal_mailbox_message(&self) -> bool {
        is_accepted_mailbox_message(
            &self.direction,
            &self.status,
            self.detail.security_disposition.as_db_value(),
        )
    }

    fn summary(&self) -> MailboxMessageSummary {
        MailboxMessageSummary {
            id: self.detail.id.clone(),
            thread_id: self.detail.thread_id.clone(),
            from_address: self.detail.from_address.clone(),
            from_display_name: self.detail.from_display_name.clone(),
            subject: self.detail.subject.clone(),
            snippet: body_snippet(&self.detail.body_text, DEFAULT_SNIPPET_CHARS),
            received_at: self.detail.received_at.clone(),
            is_read: self.detail.is_read,
            has_attachments: !self.detail.attachments.is_empty(),
            attachment_count: self.detail.attachments.len() as i32,
            contact_id: self.detail.contact_id.clone(),
            auth_verdict: self.detail.auth_verdict,
            spam_result: self.detail.spam_result,
            virus_result: self.detail.virus_result,
            security_disposition: self.detail.security_disposition,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryMailboxService {
    messages: Arc<Mutex<BTreeMap<String, InMemoryMailboxMessage>>>,
}

impl InMemoryMailboxService {
    pub fn with_messages(messages: impl IntoIterator<Item = InMemoryMailboxMessage>) -> Self {
        Self {
            messages: Arc::new(Mutex::new(
                messages
                    .into_iter()
                    .map(|message| (message.detail.id.clone(), message))
                    .collect(),
            )),
        }
    }
}

#[async_trait]
impl MailboxService for InMemoryMailboxService {
    async fn list_messages(&self, query: MailboxQuery) -> AppResult<Vec<MailboxMessageSummary>> {
        let limit = query.validated_limit()? as usize;
        let unread_only = query.unread_only.unwrap_or(false);
        let before_received_at = query.before_received_at;
        let mut messages = self
            .messages
            .lock()
            .unwrap()
            .values()
            .filter(|message| message.is_normal_mailbox_message())
            .filter(|message| !unread_only || !message.detail.is_read)
            .filter(|message| {
                before_received_at
                    .as_ref()
                    .and_then(|before| {
                        message
                            .detail
                            .received_at
                            .as_ref()
                            .map(|received_at| received_at < before)
                    })
                    .unwrap_or(true)
            })
            .map(InMemoryMailboxMessage::summary)
            .collect::<Vec<_>>();
        sort_summaries_desc(&mut messages);
        messages.truncate(limit);
        Ok(messages)
    }

    async fn get_message(&self, message_id: &str) -> AppResult<MailboxMessageDetail> {
        self.messages
            .lock()
            .unwrap()
            .get(message_id)
            .filter(|message| message.is_normal_mailbox_message())
            .map(|message| message.detail.clone())
            .ok_or_else(|| AppError::NotFound(format!("message {message_id}")))
    }

    async fn get_thread(&self, thread_id: &str) -> AppResult<MailboxThreadDetail> {
        let mut messages = self
            .messages
            .lock()
            .unwrap()
            .values()
            .filter(|message| message.is_normal_mailbox_message())
            .filter(|message| message.detail.thread_id.as_deref() == Some(thread_id))
            .cloned()
            .collect::<Vec<_>>();
        messages.sort_by(|left, right| {
            left.detail
                .received_at
                .cmp(&right.detail.received_at)
                .then_with(|| left.detail.id.cmp(&right.detail.id))
        });

        let first = messages
            .first()
            .ok_or_else(|| AppError::NotFound(format!("thread {thread_id}")))?;
        Ok(MailboxThreadDetail {
            thread_id: thread_id.to_string(),
            normalized_subject: first.normalized_subject.clone(),
            message_count: messages.len() as i32,
            last_activity_at: messages
                .iter()
                .filter_map(|message| message.last_activity_at.clone())
                .max(),
            messages: messages.into_iter().map(|message| message.detail).collect(),
        })
    }

    async fn search_messages(
        &self,
        query: MailboxSearchQuery,
    ) -> AppResult<Vec<MailboxMessageSummary>> {
        let query = query.validated()?;
        let needle = query.q.to_ascii_lowercase();
        let mut messages = self
            .messages
            .lock()
            .unwrap()
            .values()
            .filter(|message| message.is_normal_mailbox_message())
            .filter(|message| {
                let detail = &message.detail;
                detail.subject.to_ascii_lowercase().contains(&needle)
                    || detail.from_address.to_ascii_lowercase().contains(&needle)
                    || detail
                        .from_display_name
                        .to_ascii_lowercase()
                        .contains(&needle)
                    || detail.body_text.to_ascii_lowercase().contains(&needle)
            })
            .map(InMemoryMailboxMessage::summary)
            .collect::<Vec<_>>();
        sort_summaries_desc(&mut messages);
        messages.truncate(query.limit as usize);
        Ok(messages)
    }

    async fn update_message_state(
        &self,
        message_id: &str,
        request: UpdateMessageStateRequest,
    ) -> AppResult<MailboxMessageSummary> {
        let mut messages = self.messages.lock().unwrap();
        let message = messages
            .get_mut(message_id)
            .filter(|message| message.is_normal_mailbox_message())
            .ok_or_else(|| AppError::NotFound(format!("message {message_id}")))?;
        message.detail.is_read = request.is_read;
        Ok(message.summary())
    }

    async fn link_message_contact(
        &self,
        message_id: &str,
        request: LinkMessageContactRequest,
    ) -> AppResult<MailboxMessageSummary> {
        request.validated_contact_id()?;
        let mut messages = self.messages.lock().unwrap();
        let message = messages
            .get_mut(message_id)
            .filter(|message| message.is_normal_mailbox_message())
            .ok_or_else(|| AppError::NotFound(format!("message {message_id}")))?;
        message.detail.contact_id = request
            .contact_id
            .map(|contact_id| contact_id.trim().to_string())
            .filter(|contact_id| !contact_id.is_empty());
        Ok(message.summary())
    }
}

fn sort_summaries_desc(messages: &mut [MailboxMessageSummary]) {
    messages.sort_by(|left, right| {
        right
            .received_at
            .cmp(&left.received_at)
            .then_with(|| right.id.cmp(&left.id))
    });
}

fn parse_uuid(value: &str, label: &'static str) -> AppResult<Uuid> {
    Uuid::parse_str(value).map_err(|_| AppError::Validation(format!("{label} must be a UUID")))
}

pub fn body_snippet(body_text: &str, max_chars: usize) -> String {
    let normalized = body_text.split_whitespace().collect::<Vec<_>>().join(" ");
    if max_chars == 0 || normalized.chars().count() <= max_chars {
        return normalized;
    }

    let mut snippet = normalized.chars().take(max_chars).collect::<String>();
    snippet.push_str("...");
    snippet
}

pub fn sanitize_attachment_filename(filename: &str) -> String {
    let without_controls = filename
        .chars()
        .filter(|ch| !ch.is_control())
        .collect::<String>();
    let normalized_separators = without_controls.replace('\\', "/");
    let basename = normalized_separators
        .split('/')
        .rev()
        .find(|part| !part.trim().is_empty())
        .unwrap_or("")
        .trim();

    let safe = basename
        .chars()
        .filter(|ch| *ch != '/' && *ch != '\\')
        .take(240)
        .collect::<String>();

    if safe.is_empty() || safe == "." || safe == ".." {
        "attachment".to_string()
    } else {
        safe
    }
}

pub fn is_accepted_mailbox_message(
    direction: &str,
    status: &str,
    security_disposition: &str,
) -> bool {
    direction.eq_ignore_ascii_case("inbound")
        && status.eq_ignore_ascii_case("received")
        && security_disposition.eq_ignore_ascii_case("accepted")
}

pub fn parse_optional_auth_result(value: Option<&str>) -> AppResult<Option<MailboxAuthResult>> {
    parse_optional_db_value(value, MailboxAuthResult::parse, "auth result")
}

pub fn parse_optional_scan_result(value: Option<&str>) -> AppResult<Option<MailboxScanResult>> {
    parse_optional_db_value(value, MailboxScanResult::parse, "scan result")
}

pub fn parse_security_disposition(value: &str) -> AppResult<MailboxSecurityDisposition> {
    MailboxSecurityDisposition::parse(value).ok_or_else(|| {
        AppError::Internal(format!(
            "unknown mailbox security disposition value {value:?}"
        ))
    })
}

pub fn parse_recipient_kind(value: &str) -> AppResult<MailboxRecipientKind> {
    match value.trim().to_ascii_lowercase().as_str() {
        "to" => Ok(MailboxRecipientKind::To),
        "cc" => Ok(MailboxRecipientKind::Cc),
        "bcc" => Ok(MailboxRecipientKind::Bcc),
        _ => Err(AppError::Internal(format!(
            "unknown recipient kind value {value:?}"
        ))),
    }
}

fn parse_optional_db_value<T>(
    value: Option<&str>,
    parse: impl Fn(&str) -> Option<T>,
    label: &'static str,
) -> AppResult<Option<T>> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            parse(value).ok_or_else(|| {
                AppError::Internal(format!("unknown mailbox {label} value {value:?}"))
            })
        })
        .transpose()
}

fn validate_limit(limit: Option<u32>) -> AppResult<u32> {
    let limit = limit.unwrap_or(DEFAULT_MAILBOX_LIMIT);
    if limit == 0 || limit > MAX_MAILBOX_LIMIT {
        return Err(AppError::Validation(format!(
            "limit must be between 1 and {MAX_MAILBOX_LIMIT}"
        )));
    }
    Ok(limit)
}

#[cfg(test)]
mod mailbox_types_tests {
    use super::{
        MailboxAuthResult, MailboxScanResult, MailboxSearchQuery, MailboxSecurityDisposition,
        body_snippet, is_accepted_mailbox_message, parse_optional_auth_result,
        parse_optional_scan_result, parse_security_disposition, sanitize_attachment_filename,
    };
    use crate::error::AppError;

    #[test]
    fn mailbox_types_accepts_only_normal_inbound_messages() {
        assert!(is_accepted_mailbox_message(
            "inbound", "received", "accepted"
        ));
        assert!(!is_accepted_mailbox_message(
            "inbound",
            "quarantined",
            "accepted"
        ));
        assert!(!is_accepted_mailbox_message(
            "inbound",
            "received",
            "quarantined"
        ));
        assert!(!is_accepted_mailbox_message(
            "outbound", "received", "accepted"
        ));
    }

    #[test]
    fn mailbox_types_builds_plaintext_snippets_without_html_interpretation() {
        let snippet = body_snippet("hello\n\n<script>alert(1)</script> friend", 24);

        assert_eq!(snippet, "hello <script>alert(1)</...");
    }

    #[test]
    fn mailbox_types_sanitizes_untrusted_attachment_filenames() {
        assert_eq!(
            sanitize_attachment_filename("../secret/\u{0}invoice.pdf"),
            "invoice.pdf"
        );
        assert_eq!(
            sanitize_attachment_filename(r"C:\fakepath\payload.exe"),
            "payload.exe"
        );
        assert_eq!(sanitize_attachment_filename("..\n"), "attachment");
    }

    #[test]
    fn mailbox_types_parses_auth_security_values() {
        assert_eq!(
            parse_optional_auth_result(Some("softfail")).unwrap(),
            Some(MailboxAuthResult::Softfail)
        );
        assert_eq!(
            parse_optional_scan_result(Some("processingFailed")).unwrap(),
            Some(MailboxScanResult::ProcessingFailed)
        );
        assert_eq!(
            parse_security_disposition("accepted").unwrap(),
            MailboxSecurityDisposition::Accepted
        );
        assert!(parse_optional_auth_result(None).unwrap().is_none());
    }

    #[test]
    fn mailbox_types_rejects_unknown_db_values() {
        assert!(matches!(
            parse_optional_auth_result(Some("mystery")).unwrap_err(),
            AppError::Internal(_)
        ));
        assert!(matches!(
            parse_security_disposition("maybe").unwrap_err(),
            AppError::Internal(_)
        ));
    }

    #[test]
    fn mailbox_types_validates_search_requests() {
        let validated = MailboxSearchQuery {
            q: "  invoice  ".to_string(),
            limit: Some(10),
        }
        .validated()
        .unwrap();

        assert_eq!(validated.q, "invoice");
        assert_eq!(validated.limit, 10);

        assert!(matches!(
            MailboxSearchQuery {
                q: " ".to_string(),
                limit: None,
            }
            .validated()
            .unwrap_err(),
            AppError::Validation(_)
        ));
        assert!(matches!(
            MailboxSearchQuery {
                q: "invoice".to_string(),
                limit: Some(101),
            }
            .validated()
            .unwrap_err(),
            AppError::Validation(_)
        ));
    }
}

#[cfg(test)]
mod mailbox_service_tests {
    use super::{
        InMemoryMailboxMessage, InMemoryMailboxService, LinkMessageContactRequest,
        MailboxAttachment, MailboxAuthResult, MailboxMessageDetail, MailboxQuery,
        MailboxScanResult, MailboxSearchQuery, MailboxSecurityDisposition, MailboxService,
        UpdateMessageStateRequest,
    };
    use crate::error::AppError;

    const ACCEPTED_ID: &str = "00000000-0000-0000-0000-000000000001";
    const QUARANTINED_ID: &str = "00000000-0000-0000-0000-000000000002";
    const REJECTED_ID: &str = "00000000-0000-0000-0000-000000000003";
    const THREAD_ID: &str = "00000000-0000-0000-0000-000000000101";
    const CONTACT_ID: &str = "00000000-0000-0000-0000-000000000201";

    fn detail(
        id: &str,
        subject: &str,
        body_text: &str,
        disposition: MailboxSecurityDisposition,
    ) -> MailboxMessageDetail {
        MailboxMessageDetail {
            id: id.to_string(),
            thread_id: Some(THREAD_ID.to_string()),
            rfc_message_id: Some(format!("<{id}@example.test>")),
            in_reply_to: None,
            reference_ids: vec![],
            from_address: "sender@example.test".to_string(),
            from_display_name: "Sender".to_string(),
            subject: subject.to_string(),
            message_date: Some("2026-01-01 00:00:00+00".to_string()),
            received_at: Some("2026-01-01 00:00:00+00".to_string()),
            body_text: body_text.to_string(),
            recipients: vec![],
            attachments: vec![MailboxAttachment {
                id: "00000000-0000-0000-0000-000000000301".to_string(),
                position: 0,
                filename: "../invoice.pdf".to_string(),
                display_filename: "invoice.pdf".to_string(),
                content_type: "application/pdf".to_string(),
                size_bytes: Some(10),
                content_id: None,
            }],
            is_read: false,
            contact_id: None,
            spf_result: Some(MailboxAuthResult::Pass),
            dkim_result: Some(MailboxAuthResult::Pass),
            dmarc_result: Some(MailboxAuthResult::Pass),
            auth_verdict: Some(MailboxAuthResult::Pass),
            spam_result: Some(MailboxScanResult::Pass),
            virus_result: Some(MailboxScanResult::Pass),
            security_disposition: disposition,
            security_reason: Some("clean".to_string()),
        }
    }

    fn service() -> InMemoryMailboxService {
        InMemoryMailboxService::with_messages([
            InMemoryMailboxMessage::accepted(detail(
                ACCEPTED_ID,
                "Invoice",
                "Plaintext invoice body",
                MailboxSecurityDisposition::Accepted,
            )),
            InMemoryMailboxMessage {
                direction: "inbound".to_string(),
                status: "quarantined".to_string(),
                normalized_subject: "invoice".to_string(),
                last_activity_at: Some("2026-01-01 00:01:00+00".to_string()),
                detail: detail(
                    QUARANTINED_ID,
                    "Invoice",
                    "Quarantined invoice body",
                    MailboxSecurityDisposition::Quarantined,
                ),
            },
            InMemoryMailboxMessage {
                direction: "inbound".to_string(),
                status: "rejected".to_string(),
                normalized_subject: "invoice".to_string(),
                last_activity_at: Some("2026-01-01 00:02:00+00".to_string()),
                detail: detail(
                    REJECTED_ID,
                    "Invoice",
                    "Rejected invoice body",
                    MailboxSecurityDisposition::Rejected,
                ),
            },
        ])
    }

    #[tokio::test]
    async fn mailbox_service_lists_accepted_messages_only() {
        let messages = service()
            .list_messages(MailboxQuery {
                limit: None,
                unread_only: None,
                before_received_at: None,
            })
            .await
            .unwrap();

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, ACCEPTED_ID);
        assert_eq!(messages[0].snippet, "Plaintext invoice body");
    }

    #[tokio::test]
    async fn mailbox_service_gets_detail_and_thread_without_quarantine() {
        let service = service();
        let detail = service.get_message(ACCEPTED_ID).await.unwrap();
        let thread = service.get_thread(THREAD_ID).await.unwrap();

        assert_eq!(detail.from_address, "sender@example.test");
        assert_eq!(detail.auth_verdict, Some(MailboxAuthResult::Pass));
        assert_eq!(thread.message_count, 1);
        assert_eq!(thread.messages[0].id, ACCEPTED_ID);

        let quarantined = service.get_message(QUARANTINED_ID).await.unwrap_err();
        assert!(matches!(quarantined, AppError::NotFound(_)));
    }

    #[tokio::test]
    async fn mailbox_service_searches_confirmed_scope() {
        let messages = service()
            .search_messages(MailboxSearchQuery {
                q: "invoice".to_string(),
                limit: None,
            })
            .await
            .unwrap();

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, ACCEPTED_ID);
    }

    #[tokio::test]
    async fn mailbox_service_updates_read_state_and_contact_links() {
        let service = service();
        let read = service
            .update_message_state(ACCEPTED_ID, UpdateMessageStateRequest { is_read: true })
            .await
            .unwrap();
        let linked = service
            .link_message_contact(
                ACCEPTED_ID,
                LinkMessageContactRequest {
                    contact_id: Some(CONTACT_ID.to_string()),
                },
            )
            .await
            .unwrap();
        let unlinked = service
            .link_message_contact(ACCEPTED_ID, LinkMessageContactRequest { contact_id: None })
            .await
            .unwrap();

        assert!(read.is_read);
        assert_eq!(linked.contact_id.as_deref(), Some(CONTACT_ID));
        assert!(unlinked.contact_id.is_none());
        assert!(matches!(
            service
                .update_message_state(QUARANTINED_ID, UpdateMessageStateRequest { is_read: true })
                .await
                .unwrap_err(),
            AppError::NotFound(_)
        ));
        assert!(matches!(
            service
                .link_message_contact(
                    ACCEPTED_ID,
                    LinkMessageContactRequest {
                        contact_id: Some("not-a-uuid".to_string()),
                    },
                )
                .await
                .unwrap_err(),
            AppError::Validation(_)
        ));
    }
}
