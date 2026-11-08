use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
};
use base64::Engine;
use serde::Deserialize;
use shared::attachment_storage::SignedAttachmentUrl;
use shared::attachment_uploads::{
    AttachmentUploadGrant, CreateAttachmentUpload, UploadedAttachmentRef,
};
use shared::outbound::{ComposeMessageRequest, OutboundMessageQueued, ReplyMessageRequest};

use crate::{ApiError, ApiState, require_user, user_context};

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/attachments/uploads", post(create_upload))
        .route("/outbound/messages/compose", post(compose))
        .route("/mailbox/messages/{message_id}/reply", post(reply))
        .route(
            "/outbound/messages/{message_id}/attachments/{position}/download",
            get(download_outbound),
        )
        .route(
            "/mailbox/messages/{message_id}/attachments/{attachment_id}/download",
            get(download_inbound),
        )
        .route(
            "/mailbox/messages/{message_id}/attachments/{attachment_id}",
            get(download_inbound),
        )
}

async fn create_upload(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateAttachmentUpload>,
) -> Result<Json<AttachmentUploadGrant>, ApiError> {
    require_user(&state, &headers).await?;
    let user = user_context(&state, &headers).await?;
    Ok(Json(
        state.attachment_uploads.create(&user.sub, request).await?,
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MessageRequest {
    from_address: String,
    #[serde(default)]
    to: Vec<String>,
    #[serde(default)]
    cc: Vec<String>,
    #[serde(default)]
    bcc: Vec<String>,
    #[serde(default)]
    subject: String,
    body_text: String,
    #[serde(default)]
    attachments: Vec<UploadedAttachmentRef>,
}

async fn compose(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<MessageRequest>,
) -> Result<Json<OutboundMessageQueued>, ApiError> {
    require_user(&state, &headers).await?;
    let user = user_context(&state, &headers).await?;
    let attachments = state
        .attachment_uploads
        .resolve(&user.sub, &request.attachments)
        .await?;
    Ok(Json(
        state
            .outbound
            .compose_message(ComposeMessageRequest {
                from_address: request.from_address,
                to: request.to,
                cc: request.cc,
                bcc: request.bcc,
                subject: request.subject,
                body_text: request.body_text,
                attachments,
            })
            .await?,
    ))
}

async fn reply(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(message_id): Path<String>,
    Json(request): Json<MessageRequest>,
) -> Result<Json<OutboundMessageQueued>, ApiError> {
    require_user(&state, &headers).await?;
    let user = user_context(&state, &headers).await?;
    let attachments = state
        .attachment_uploads
        .resolve(&user.sub, &request.attachments)
        .await?;
    Ok(Json(
        state
            .outbound
            .reply_to_message(
                &message_id,
                ReplyMessageRequest {
                    from_address: request.from_address,
                    to: request.to,
                    cc: request.cc,
                    bcc: request.bcc,
                    body_text: request.body_text,
                    attachments,
                },
            )
            .await?,
    ))
}

async fn download_inbound(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path((message_id, attachment_id)): Path<(String, String)>,
) -> Result<Json<SignedAttachmentUrl>, ApiError> {
    require_user(&state, &headers).await?;
    // Extraction stays server-side. The authenticated API returns only a signed
    // link; file bytes and untrusted MIME never cross the application edge.
    let attachment = state
        .attachments
        .download_attachment(&message_id, &attachment_id)
        .await?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&attachment.content_base64)
        .map_err(|_| {
            shared::error::AppError::Internal("invalid stored attachment payload".into())
        })?;
    Ok(Json(
        state
            .attachment_storage
            .cache_download(
                &format!("attachments/downloads/{}", uuid::Uuid::new_v4()),
                bytes,
                &attachment.display_filename,
            )
            .await?,
    ))
}

async fn download_outbound(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path((message_id, position)): Path<(String, i32)>,
) -> Result<Json<SignedAttachmentUrl>, ApiError> {
    require_user(&state, &headers).await?;
    let detail = state.outbound.get_outbound_message(&message_id).await?;
    let attachment = detail
        .attachments
        .iter()
        .find(|a| a.position == position)
        .ok_or_else(|| shared::error::AppError::NotFound("attachment".into()))?;
    if let Some(object) = &attachment.storage {
        return Ok(Json(
            state
                .attachment_storage
                .download(object, &attachment.display_filename)
                .await?,
        ));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&attachment.content_base64)
        .map_err(|_| {
            shared::error::AppError::Internal("invalid stored attachment payload".into())
        })?;
    Ok(Json(
        state
            .attachment_storage
            .cache_download(
                &format!("attachments/downloads/{}", uuid::Uuid::new_v4()),
                bytes,
                &attachment.display_filename,
            )
            .await?,
    ))
}
