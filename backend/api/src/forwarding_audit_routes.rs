use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::routing::get;
use axum::{Json, Router};
use shared::forwarding_audit::{
    ForwardingAuditQuery, ForwardingMessageStatus, ForwardingRuleStatus, PgForwardingAuditService,
};

use crate::{ApiError, ApiState, require_user};

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/forwarding/audit/rules", get(list_rule_statuses))
        .route("/forwarding/audit/messages", get(list_message_statuses))
}

async fn list_rule_statuses(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ForwardingRuleStatus>>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(service(&state).list_rule_statuses().await?))
}

async fn list_message_statuses(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<ForwardingAuditQuery>,
) -> Result<Json<Vec<ForwardingMessageStatus>>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(service(&state).list_message_statuses(query).await?))
}

fn service(state: &ApiState) -> PgForwardingAuditService {
    PgForwardingAuditService::new(state.db.clone())
}
