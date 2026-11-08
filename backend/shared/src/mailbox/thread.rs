use super::{
    AppError, AppResult, MailboxMessageDetail, MailboxThreadDetail, MessageDetailRow,
    PgMailboxService, ThreadRow, parse_uuid,
};

impl PgMailboxService {
    pub(super) async fn fetch_thread(&self, thread_id: &str) -> AppResult<MailboxThreadDetail> {
        let thread_id = parse_uuid(thread_id, "thread id")?;
        let row: Option<ThreadRow> = sqlx::query_as(
            "SELECT id, normalized_subject, last_activity_at::text AS last_activity_at
             FROM threads WHERE id = $1",
        )
        .bind(thread_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;
        let row = row.ok_or_else(|| AppError::NotFound(format!("thread {thread_id}")))?;
        let rows: Vec<MessageDetailRow> = sqlx::query_as(include_str!("thread.sql"))
            .bind(thread_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        let mut messages = Vec::with_capacity(rows.len());
        for detail in rows {
            messages.push(self.detail_from_row(detail).await?);
        }
        if messages.is_empty() {
            return Err(AppError::NotFound(format!("thread {thread_id}")));
        }
        Ok(MailboxThreadDetail {
            thread_id: row.id.to_string(),
            normalized_subject: row.normalized_subject,
            message_count: messages.len() as i32,
            last_activity_at: messages
                .last()
                .and_then(|message| message_timestamp(message).cloned())
                .or(row.last_activity_at),
            messages,
        })
    }
}

pub(super) fn message_timestamp(message: &MailboxMessageDetail) -> Option<&String> {
    message
        .received_at
        .as_ref()
        .or(message.sent_at.as_ref())
        .or(message.created_at.as_ref())
}
