use async_trait::async_trait;
use uuid::Uuid;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::finance_model::{
    FinanceCategoryTotalRow, FinanceExpenseRow, FinanceReceivableRow, FinanceVendorTotalRow,
    NormalizedExpenseInput, NormalizedReceivableInput, parse_optional_uuid, parse_uuid,
};
use crate::finance_sql::{
    CATEGORY_TOTALS, EXPENSE_INSERT, EXPENSE_LIST, EXPENSE_SELECT_BY_ID, EXPENSE_TOTALS,
    EXPENSE_UPDATE, RECEIVABLE_INSERT, RECEIVABLE_LIST, RECEIVABLE_SELECT_BY_ID, RECEIVABLE_TOTALS,
    RECEIVABLE_UPDATE, VENDOR_TOTALS,
};

pub use crate::finance_types::{
    CreateFinanceExpenseRequest, CreateFinanceReceivableRequest, ExpenseKind, ExpenseStatus,
    FinanceCategoryTotal, FinanceExpense, FinanceExpenseQuery, FinanceReceivable,
    FinanceReceivableQuery, FinanceSummary, FinanceSummaryQuery, FinanceVendorTotal,
    ReceivableStatus, RecurrenceInterval, UpdateFinanceExpenseRequest,
    UpdateFinanceReceivableRequest,
};

#[async_trait]
pub trait FinanceService: Send + Sync {
    async fn list_expenses(&self, query: FinanceExpenseQuery) -> AppResult<Vec<FinanceExpense>>;
    async fn create_expense(
        &self,
        request: CreateFinanceExpenseRequest,
    ) -> AppResult<FinanceExpense>;
    async fn update_expense(
        &self,
        expense_id: &str,
        request: UpdateFinanceExpenseRequest,
    ) -> AppResult<FinanceExpense>;
    async fn list_receivables(
        &self,
        query: FinanceReceivableQuery,
    ) -> AppResult<Vec<FinanceReceivable>>;
    async fn create_receivable(
        &self,
        request: CreateFinanceReceivableRequest,
    ) -> AppResult<FinanceReceivable>;
    async fn update_receivable(
        &self,
        receivable_id: &str,
        request: UpdateFinanceReceivableRequest,
    ) -> AppResult<FinanceReceivable>;
    async fn summary(&self, query: FinanceSummaryQuery) -> AppResult<FinanceSummary>;
}

#[derive(Debug, Clone)]
pub struct PgFinanceService {
    pool: DbPool,
}

impl PgFinanceService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    async fn expense_row(&self, expense_id: Uuid) -> AppResult<FinanceExpenseRow> {
        sqlx::query_as(EXPENSE_SELECT_BY_ID)
            .bind(expense_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?
            .ok_or_else(|| AppError::NotFound(format!("finance expense {expense_id}")))
    }

    async fn receivable_row(&self, receivable_id: Uuid) -> AppResult<FinanceReceivableRow> {
        sqlx::query_as(RECEIVABLE_SELECT_BY_ID)
            .bind(receivable_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?
            .ok_or_else(|| AppError::NotFound(format!("finance receivable {receivable_id}")))
    }
}

#[async_trait]
impl FinanceService for PgFinanceService {
    async fn list_expenses(&self, query: FinanceExpenseQuery) -> AppResult<Vec<FinanceExpense>> {
        let status = query.status.map(ExpenseStatus::as_db_value);
        let category = query
            .category
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty());
        let limit = query.limit.unwrap_or(100).clamp(1, 250);
        let rows: Vec<FinanceExpenseRow> = sqlx::query_as(EXPENSE_LIST)
            .bind(query.tax_year)
            .bind(status)
            .bind(category)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn create_expense(
        &self,
        request: CreateFinanceExpenseRequest,
    ) -> AppResult<FinanceExpense> {
        let input = NormalizedExpenseInput::create(request)?;
        let row = bind_expense(sqlx::query_as(EXPENSE_INSERT), &input)
            .fetch_one(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        FinanceExpenseRow::try_into(row)
    }

    async fn update_expense(
        &self,
        expense_id: &str,
        request: UpdateFinanceExpenseRequest,
    ) -> AppResult<FinanceExpense> {
        let expense_id = parse_uuid(expense_id, "expense id")?;
        let input = NormalizedExpenseInput::update(self.expense_row(expense_id).await?, request)?;
        let row = bind_expense(sqlx::query_as(EXPENSE_UPDATE).bind(expense_id), &input)
            .fetch_one(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        FinanceExpenseRow::try_into(row)
    }

    async fn list_receivables(
        &self,
        query: FinanceReceivableQuery,
    ) -> AppResult<Vec<FinanceReceivable>> {
        let contact_id = parse_optional_uuid(query.contact_id.as_deref(), "contact id")?;
        let status = query.status.map(ReceivableStatus::as_db_value);
        let limit = query.limit.unwrap_or(100).clamp(1, 250);
        let rows: Vec<FinanceReceivableRow> = sqlx::query_as(RECEIVABLE_LIST)
            .bind(contact_id)
            .bind(status)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn create_receivable(
        &self,
        request: CreateFinanceReceivableRequest,
    ) -> AppResult<FinanceReceivable> {
        let input = NormalizedReceivableInput::create(request)?;
        let row = bind_receivable(sqlx::query_as(RECEIVABLE_INSERT), &input)
            .fetch_one(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        FinanceReceivableRow::try_into(row)
    }

    async fn update_receivable(
        &self,
        receivable_id: &str,
        request: UpdateFinanceReceivableRequest,
    ) -> AppResult<FinanceReceivable> {
        let receivable_id = parse_uuid(receivable_id, "receivable id")?;
        let input =
            NormalizedReceivableInput::update(self.receivable_row(receivable_id).await?, request)?;
        let row = bind_receivable(
            sqlx::query_as(RECEIVABLE_UPDATE).bind(receivable_id),
            &input,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|err| AppError::Database(err.to_string()))?;
        FinanceReceivableRow::try_into(row)
    }

    async fn summary(&self, query: FinanceSummaryQuery) -> AppResult<FinanceSummary> {
        let tax_year = query.tax_year.unwrap_or_else(current_tax_year);
        let expenses: FinanceExpenseTotalsRow = sqlx::query_as(EXPENSE_TOTALS)
            .bind(tax_year)
            .fetch_one(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        let receivables: FinanceReceivableTotalsRow = sqlx::query_as(RECEIVABLE_TOTALS)
            .fetch_one(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?;
        let category_totals = sqlx::query_as::<_, FinanceCategoryTotalRow>(CATEGORY_TOTALS)
            .bind(tax_year)
            .fetch_all(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?
            .into_iter()
            .map(Into::into)
            .collect();
        let vendor_totals = sqlx::query_as::<_, FinanceVendorTotalRow>(VENDOR_TOTALS)
            .bind(tax_year)
            .fetch_all(&self.pool)
            .await
            .map_err(|err| AppError::Database(err.to_string()))?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(FinanceSummary {
            tax_year,
            gross_expense_cents: expenses.gross_cents,
            business_expense_cents: expenses.business_cents,
            personal_expense_cents: expenses.personal_cents,
            receivable_owed_cents: receivables.owed_cents,
            receivable_paid_cents: receivables.paid_cents,
            category_totals,
            vendor_totals,
        })
    }
}

impl TryFrom<FinanceExpenseRow> for FinanceExpense {
    type Error = AppError;

    fn try_from(value: FinanceExpenseRow) -> AppResult<Self> {
        let business_amount_cents =
            business_cents(value.amount_cents, value.business_use_percent_bps);
        Ok(Self {
            id: value.id.to_string(),
            title: value.title,
            vendor_name: value.vendor_name,
            category: value.category,
            expense_kind: ExpenseKind::parse(&value.expense_kind)?,
            recurrence_interval: RecurrenceInterval::parse(&value.recurrence_interval)?,
            status: ExpenseStatus::parse(&value.status)?,
            amount_cents: value.amount_cents,
            business_amount_cents,
            personal_amount_cents: value.amount_cents - business_amount_cents,
            currency: value.currency,
            incurred_on: value.incurred_on,
            service_period_start: value.service_period_start,
            service_period_end: value.service_period_end,
            business_use_percent_bps: value.business_use_percent_bps,
            source_message_id: value.source_message_id.map(|id| id.to_string()),
            source_attachment_id: value.source_attachment_id.map(|id| id.to_string()),
            source_asset_id: value.source_asset_id,
            notes: value.notes,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

impl TryFrom<FinanceReceivableRow> for FinanceReceivable {
    type Error = AppError;

    fn try_from(value: FinanceReceivableRow) -> AppResult<Self> {
        Ok(Self {
            id: value.id.to_string(),
            contact_id: value.contact_id.map(|id| id.to_string()),
            title: value.title,
            status: ReceivableStatus::parse(&value.status)?,
            amount_cents: value.amount_cents,
            currency: value.currency,
            issued_on: value.issued_on,
            due_on: value.due_on,
            paid_on: value.paid_on,
            source_message_id: value.source_message_id.map(|id| id.to_string()),
            source_booking_id: value.source_booking_id.map(|id| id.to_string()),
            source_asset_id: value.source_asset_id,
            external_reference: value.external_reference,
            notes: value.notes,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

impl From<FinanceCategoryTotalRow> for FinanceCategoryTotal {
    fn from(value: FinanceCategoryTotalRow) -> Self {
        Self {
            category: value.category,
            gross_cents: value.gross_cents,
            business_cents: value.business_cents,
            personal_cents: value.personal_cents,
        }
    }
}

impl From<FinanceVendorTotalRow> for FinanceVendorTotal {
    fn from(value: FinanceVendorTotalRow) -> Self {
        Self {
            vendor_name: value.vendor_name,
            gross_cents: value.gross_cents,
            business_cents: value.business_cents,
            personal_cents: value.personal_cents,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct FinanceExpenseTotalsRow {
    gross_cents: i64,
    business_cents: i64,
    personal_cents: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct FinanceReceivableTotalsRow {
    owed_cents: i64,
    paid_cents: i64,
}

type ExpenseQuery<'q> =
    sqlx::query::QueryAs<'q, sqlx::Postgres, FinanceExpenseRow, sqlx::postgres::PgArguments>;
type ReceivableQuery<'q> =
    sqlx::query::QueryAs<'q, sqlx::Postgres, FinanceReceivableRow, sqlx::postgres::PgArguments>;

fn bind_expense<'q>(
    query: ExpenseQuery<'q>,
    input: &'q NormalizedExpenseInput,
) -> ExpenseQuery<'q> {
    query
        .bind(&input.title)
        .bind(&input.vendor_name)
        .bind(&input.category)
        .bind(input.expense_kind.as_db_value())
        .bind(input.recurrence_interval.as_db_value())
        .bind(input.status.as_db_value())
        .bind(input.amount_cents)
        .bind(&input.currency)
        .bind(&input.incurred_on)
        .bind(&input.service_period_start)
        .bind(&input.service_period_end)
        .bind(input.business_use_percent_bps)
        .bind(input.source_message_id)
        .bind(input.source_attachment_id)
        .bind(&input.source_asset_id)
        .bind(&input.notes)
}

fn bind_receivable<'q>(
    query: ReceivableQuery<'q>,
    input: &'q NormalizedReceivableInput,
) -> ReceivableQuery<'q> {
    query
        .bind(input.contact_id)
        .bind(&input.title)
        .bind(input.status.as_db_value())
        .bind(input.amount_cents)
        .bind(&input.currency)
        .bind(&input.issued_on)
        .bind(&input.due_on)
        .bind(&input.paid_on)
        .bind(input.source_message_id)
        .bind(input.source_booking_id)
        .bind(&input.source_asset_id)
        .bind(&input.external_reference)
        .bind(&input.notes)
}

fn business_cents(amount_cents: i64, bps: i32) -> i64 {
    amount_cents * i64::from(bps) / 10000
}

fn current_tax_year() -> i32 {
    time::OffsetDateTime::now_utc().year()
}
