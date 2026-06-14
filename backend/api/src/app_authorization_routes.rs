use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, put};
use axum::{Json, Router};
use shared::app_authorizations::{AppAuthorizationUser, UpsertAppAuthorizationUserRequest};
use shared::auth::UserContext;
use shared::error::AppError;

use crate::{ApiError, ApiState};

pub fn router() -> Router<ApiState> {
    Router::new()
        .route(
            "/app-authorizations/users",
            get(list_app_authorization_users),
        )
        .route(
            "/app-authorizations/users/{username}",
            put(upsert_app_authorization_user).delete(delete_app_authorization_user),
        )
}

async fn list_app_authorization_users(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AppAuthorizationUser>>, ApiError> {
    require_operator(&state, &headers).await?;
    Ok(Json(state.app_authorizations.list_users().await?))
}

async fn upsert_app_authorization_user(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(username): Path<String>,
    Json(request): Json<UpsertAppAuthorizationUserRequest>,
) -> Result<Json<AppAuthorizationUser>, ApiError> {
    require_operator(&state, &headers).await?;
    Ok(Json(
        state
            .app_authorizations
            .upsert_user(&username, request)
            .await?,
    ))
}

async fn delete_app_authorization_user(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(username): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_operator(&state, &headers).await?;
    state.app_authorizations.delete_user(&username).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn require_operator(state: &ApiState, headers: &HeaderMap) -> Result<(), AppError> {
    let user = crate::user_context(state, headers).await?;
    if is_operator(&user) {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

fn is_operator(user: &UserContext) -> bool {
    user.username.as_deref() == Some("chris") || user.groups.iter().any(|group| group == "admin")
}
