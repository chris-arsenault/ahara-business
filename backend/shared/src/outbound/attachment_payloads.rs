use super::*;

pub(super) async fn insert_outbound_attachments(
    tx: &mut Transaction<'_, Postgres>,
    message_id: Uuid,
    attachments: &[OutboundAttachment],
) -> AppResult<()> {
    for attachment in attachments {
        let row: MessageIdRow = sqlx::query_as(
            "INSERT INTO attachment_refs (
                 message_id, position, filename, content_type, size_bytes
             )
             VALUES ($1, $2, $3, $4, $5)
             RETURNING id",
        )
        .bind(message_id)
        .bind(attachment.position)
        .bind(&attachment.filename)
        .bind(&attachment.content_type)
        .bind(attachment.size_bytes)
        .fetch_one(&mut **tx)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;

        sqlx::query(
            "INSERT INTO outbound_attachment_payloads (attachment_id, content_base64, object_key, version_id)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(row.id)
        .bind(attachment.storage.is_none().then_some(&attachment.content_base64))
        .bind(attachment.storage.as_ref().map(|o| &o.key))
        .bind(attachment.storage.as_ref().map(|o| &o.version_id))
        .execute(&mut **tx)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;
    }
    Ok(())
}

pub(super) async fn fetch_outbound_attachments(
    pool: &DbPool,
    message_id: Uuid,
) -> AppResult<Vec<OutboundAttachment>> {
    let rows: Vec<OutboundAttachmentRow> = sqlx::query_as(
        "SELECT attachment_refs.position,
                attachment_refs.filename,
                attachment_refs.content_type,
                attachment_refs.size_bytes,
                outbound_attachment_payloads.content_base64,
                outbound_attachment_payloads.object_key,
                outbound_attachment_payloads.version_id
         FROM attachment_refs
         JOIN outbound_attachment_payloads
           ON outbound_attachment_payloads.attachment_id = attachment_refs.id
         WHERE attachment_refs.message_id = $1
         ORDER BY attachment_refs.position ASC, attachment_refs.id ASC",
    )
    .bind(message_id)
    .fetch_all(pool)
    .await
    .map_err(|err| AppError::Database(err.to_string()))?;
    rows.into_iter().map(TryInto::try_into).collect()
}

pub(super) async fn fetch_outbound_mime_attachments_tx(
    tx: &mut Transaction<'_, Postgres>,
    message_id: Uuid,
    storage: Option<&dyn AttachmentStorage>,
) -> AppResult<Vec<OutboundMimeAttachment>> {
    let mut result = Vec::new();
    for attachment in fetch_outbound_attachments_tx(tx, message_id).await? {
        if let Some(object) = &attachment.storage {
            let storage = storage
                .ok_or_else(|| AppError::Internal("attachment storage is not configured".into()))?;
            result.push(OutboundMimeAttachment {
                filename: attachment.filename.clone(),
                content_type: attachment.content_type.clone(),
                bytes: storage.read(object).await?,
            });
        } else {
            result.push(attachment.into_mime_attachment()?);
        }
    }
    Ok(result)
}

async fn fetch_outbound_attachments_tx(
    tx: &mut Transaction<'_, Postgres>,
    message_id: Uuid,
) -> AppResult<Vec<OutboundAttachment>> {
    let rows: Vec<OutboundAttachmentRow> = sqlx::query_as(
        "SELECT attachment_refs.position,
                attachment_refs.filename,
                attachment_refs.content_type,
                attachment_refs.size_bytes,
                outbound_attachment_payloads.content_base64,
                outbound_attachment_payloads.object_key,
                outbound_attachment_payloads.version_id
         FROM attachment_refs
         JOIN outbound_attachment_payloads
           ON outbound_attachment_payloads.attachment_id = attachment_refs.id
         WHERE attachment_refs.message_id = $1
         ORDER BY attachment_refs.position ASC, attachment_refs.id ASC",
    )
    .bind(message_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|err| AppError::Database(err.to_string()))?;
    rows.into_iter().map(TryInto::try_into).collect()
}

#[derive(Debug, sqlx::FromRow)]
struct OutboundAttachmentRow {
    position: i32,
    filename: String,
    content_type: String,
    size_bytes: Option<i64>,
    content_base64: Option<String>,
    object_key: Option<String>,
    version_id: Option<String>,
}

impl TryFrom<OutboundAttachmentRow> for OutboundAttachment {
    type Error = AppError;

    fn try_from(row: OutboundAttachmentRow) -> Result<Self, Self::Error> {
        Ok(Self {
            position: row.position,
            display_filename: crate::mailbox::sanitize_attachment_filename(&row.filename),
            filename: row.filename,
            content_type: row.content_type,
            size_bytes: row.size_bytes.unwrap_or_default(),
            content_base64: row.content_base64.unwrap_or_default(),
            storage: row
                .object_key
                .zip(row.version_id)
                .map(|(key, version_id)| StoredAttachment {
                    key,
                    version_id,
                    size_bytes: row.size_bytes.unwrap_or_default(),
                }),
        })
    }
}
