use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::routing::{get, patch};
use axum::{Json, Router};
use shared::finance::{
    CreateFinanceExpenseRequest, CreateFinanceReceivableRequest, FinanceExpense,
    FinanceExpenseQuery, FinanceReceivable, FinanceReceivableQuery, FinanceSummary,
    FinanceSummaryQuery, UpdateFinanceExpenseRequest, UpdateFinanceReceivableRequest,
};

use crate::{ApiError, ApiState, require_user};

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/finance/expenses", get(list_expenses).post(create_expense))
        .route("/finance/expenses/{expense_id}", patch(update_expense))
        .route(
            "/finance/receivables",
            get(list_receivables).post(create_receivable),
        )
        .route(
            "/finance/receivables/{receivable_id}",
            patch(update_receivable),
        )
        .route("/finance/summary", get(summary))
}

async fn list_expenses(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<FinanceExpenseQuery>,
) -> Result<Json<Vec<FinanceExpense>>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(state.finance.list_expenses(query).await?))
}

async fn create_expense(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateFinanceExpenseRequest>,
) -> Result<Json<FinanceExpense>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(state.finance.create_expense(request).await?))
}

async fn update_expense(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(expense_id): Path<String>,
    Json(request): Json<UpdateFinanceExpenseRequest>,
) -> Result<Json<FinanceExpense>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(
        state.finance.update_expense(&expense_id, request).await?,
    ))
}

async fn list_receivables(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<FinanceReceivableQuery>,
) -> Result<Json<Vec<FinanceReceivable>>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(state.finance.list_receivables(query).await?))
}

async fn create_receivable(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateFinanceReceivableRequest>,
) -> Result<Json<FinanceReceivable>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(state.finance.create_receivable(request).await?))
}

async fn update_receivable(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(receivable_id): Path<String>,
    Json(request): Json<UpdateFinanceReceivableRequest>,
) -> Result<Json<FinanceReceivable>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(
        state
            .finance
            .update_receivable(&receivable_id, request)
            .await?,
    ))
}

async fn summary(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<FinanceSummaryQuery>,
) -> Result<Json<FinanceSummary>, ApiError> {
    require_user(&state, &headers).await?;
    Ok(Json(state.finance.summary(query).await?))
}
