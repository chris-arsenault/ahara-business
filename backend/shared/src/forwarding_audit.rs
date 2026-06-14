use serde::{Deserialize, Serialize};

use crate::db::DbPool;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardingRuleStatus {
    pub rule_id: String,
    pub rule_kind: String,
    pub domain_name: String,
    pub local_part: Option<String>,
    pub target_address: String,
    pub active: bool,
    pub queued_count: i64,
    pub sending_count: i64,
    pub sent_count: i64,
    pub failed_count: i64,
    pub bounced_count: i64,
    pub complained_count: i64,
    pub last_attempt_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardingMessageStatus {
    pub source_message_id: String,
    pub thread_id: Option<String>,
    pub subject: String,
    pub from_address: String,
    pub received_at: Option<String>,
    pub matching_rule_count: i64,
    pub queued_count: i64,
    pub sending_count: i64,
    pub sent_count: i64,
    pub failed_count: i64,
    pub bounced_count: i64,
    pub complained_count: i64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ForwardingAuditQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct PgForwardingAuditService {
    pool: DbPool,
}

impl PgForwardingAuditService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn list_rule_statuses(&self) -> AppResult<Vec<ForwardingRuleStatus>> {
        let rows: Vec<ForwardingRuleStatusRow> = sqlx::query_as(RULE_STATUS_SQL)
            .fetch_all(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn list_message_statuses(
        &self,
        query: ForwardingAuditQuery,
    ) -> AppResult<Vec<ForwardingMessageStatus>> {
        let limit = query.limit.unwrap_or(100).clamp(1, 250);
        let rows: Vec<ForwardingMessageStatusRow> = sqlx::query_as(MESSAGE_STATUS_SQL)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}

#[derive(Debug, sqlx::FromRow)]
struct ForwardingRuleStatusRow {
    rule_id: uuid::Uuid,
    rule_kind: String,
    domain_name: String,
    local_part: Option<String>,
    target_address: String,
    active: bool,
    queued_count: i64,
    sending_count: i64,
    sent_count: i64,
    failed_count: i64,
    bounced_count: i64,
    complained_count: i64,
    last_attempt_at: Option<String>,
    last_error: Option<String>,
}

impl From<ForwardingRuleStatusRow> for ForwardingRuleStatus {
    fn from(value: ForwardingRuleStatusRow) -> Self {
        Self {
            rule_id: value.rule_id.to_string(),
            rule_kind: value.rule_kind,
            domain_name: value.domain_name,
            local_part: value.local_part,
            target_address: value.target_address,
            active: value.active,
            queued_count: value.queued_count,
            sending_count: value.sending_count,
            sent_count: value.sent_count,
            failed_count: value.failed_count,
            bounced_count: value.bounced_count,
            complained_count: value.complained_count,
            last_attempt_at: value.last_attempt_at,
            last_error: value.last_error,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct ForwardingMessageStatusRow {
    source_message_id: uuid::Uuid,
    thread_id: Option<uuid::Uuid>,
    subject: String,
    from_address: String,
    received_at: Option<String>,
    matching_rule_count: i64,
    queued_count: i64,
    sending_count: i64,
    sent_count: i64,
    failed_count: i64,
    bounced_count: i64,
    complained_count: i64,
    last_error: Option<String>,
}

impl From<ForwardingMessageStatusRow> for ForwardingMessageStatus {
    fn from(value: ForwardingMessageStatusRow) -> Self {
        Self {
            source_message_id: value.source_message_id.to_string(),
            thread_id: value.thread_id.map(|id| id.to_string()),
            subject: value.subject,
            from_address: value.from_address,
            received_at: value.received_at,
            matching_rule_count: value.matching_rule_count,
            queued_count: value.queued_count,
            sending_count: value.sending_count,
            sent_count: value.sent_count,
            failed_count: value.failed_count,
            bounced_count: value.bounced_count,
            complained_count: value.complained_count,
            last_error: value.last_error,
        }
    }
}

const RULE_STATUS_SQL: &str = "SELECT forwarding_rules.id AS rule_id,
        forwarding_rules.rule_kind, domains.domain_name, addresses.local_part,
        forwarding_rules.target_address, forwarding_rules.active,
        count(*) FILTER (WHERE outbound_work.status = 'queued') AS queued_count,
        count(*) FILTER (WHERE outbound_work.status = 'sending') AS sending_count,
        count(*) FILTER (WHERE outbound_work.status = 'sent') AS sent_count,
        count(*) FILTER (WHERE outbound_work.status = 'failed') AS failed_count,
        count(*) FILTER (WHERE outbound_work.status = 'bounced') AS bounced_count,
        count(*) FILTER (WHERE outbound_work.status = 'complained') AS complained_count,
        max(outbound_work.updated_at)::text AS last_attempt_at,
        (array_agg(outbound_work.last_error ORDER BY outbound_work.updated_at DESC)
            FILTER (WHERE outbound_work.last_error IS NOT NULL))[1] AS last_error
    FROM forwarding_rules
    LEFT JOIN addresses ON addresses.id = forwarding_rules.address_id
    JOIN domains ON domains.id = COALESCE(forwarding_rules.domain_id, addresses.domain_id)
    LEFT JOIN outbound_work ON outbound_work.idempotency_key LIKE 'forward:%:%'
        AND split_part(outbound_work.idempotency_key, ':', 3)::uuid = forwarding_rules.id
    GROUP BY forwarding_rules.id, forwarding_rules.rule_kind, domains.domain_name,
        addresses.local_part, forwarding_rules.target_address, forwarding_rules.active
    ORDER BY domains.domain_name ASC, forwarding_rules.rule_kind ASC,
        addresses.local_part ASC NULLS LAST, forwarding_rules.target_address ASC";

const MESSAGE_STATUS_SQL: &str = "SELECT messages.id AS source_message_id,
        messages.thread_id, messages.subject, messages.from_address,
        messages.received_at::text AS received_at,
        count(DISTINCT forwarding_rules.id) AS matching_rule_count,
        count(DISTINCT outbound_work.id) FILTER (WHERE outbound_work.status = 'queued') AS queued_count,
        count(DISTINCT outbound_work.id) FILTER (WHERE outbound_work.status = 'sending') AS sending_count,
        count(DISTINCT outbound_work.id) FILTER (WHERE outbound_work.status = 'sent') AS sent_count,
        count(DISTINCT outbound_work.id) FILTER (WHERE outbound_work.status = 'failed') AS failed_count,
        count(DISTINCT outbound_work.id) FILTER (WHERE outbound_work.status = 'bounced') AS bounced_count,
        count(DISTINCT outbound_work.id) FILTER (WHERE outbound_work.status = 'complained') AS complained_count,
        (array_agg(outbound_work.last_error ORDER BY outbound_work.updated_at DESC)
            FILTER (WHERE outbound_work.last_error IS NOT NULL))[1] AS last_error
    FROM messages
    LEFT JOIN forwarding_rules ON forwarding_rules.active = true
        AND ((forwarding_rules.rule_kind = 'address'
              AND forwarding_rules.address_id = messages.matched_address_id)
             OR (forwarding_rules.rule_kind = 'domain'
                 AND forwarding_rules.domain_id = messages.matched_domain_id))
        AND (forwarding_rules.sender_address_normalized IS NULL
             OR forwarding_rules.sender_address_normalized = messages.from_address_normalized)
        AND (forwarding_rules.plus_tag IS NULL OR forwarding_rules.plus_tag = messages.plus_tag)
    LEFT JOIN outbound_work ON outbound_work.source_message_id = messages.id
        AND outbound_work.idempotency_key LIKE 'forward:%:%'
    WHERE messages.direction = 'inbound'
      AND messages.status = 'received'
      AND messages.security_disposition = 'accepted'
    GROUP BY messages.id, messages.thread_id, messages.subject, messages.from_address,
        messages.received_at, messages.created_at
    HAVING count(DISTINCT forwarding_rules.id) > 0 OR count(outbound_work.id) > 0
    ORDER BY COALESCE(messages.received_at, messages.created_at) DESC, messages.id DESC
    LIMIT $1";
