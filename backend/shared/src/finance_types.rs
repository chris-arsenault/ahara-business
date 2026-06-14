use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpenseKind {
    OneTime,
    Recurring,
}

impl ExpenseKind {
    pub(crate) fn as_db_value(self) -> &'static str {
        match self {
            Self::OneTime => "one_time",
            Self::Recurring => "recurring",
        }
    }

    pub(crate) fn parse(value: &str) -> AppResult<Self> {
        match value {
            "one_time" => Ok(Self::OneTime),
            "recurring" => Ok(Self::Recurring),
            _ => Err(AppError::Internal(format!("unknown expense kind {value}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecurrenceInterval {
    None,
    Weekly,
    Monthly,
    Annual,
}

impl RecurrenceInterval {
    pub(crate) fn as_db_value(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
            Self::Annual => "annual",
        }
    }

    pub(crate) fn parse(value: &str) -> AppResult<Self> {
        match value {
            "none" => Ok(Self::None),
            "weekly" => Ok(Self::Weekly),
            "monthly" => Ok(Self::Monthly),
            "annual" => Ok(Self::Annual),
            _ => Err(AppError::Internal(format!("unknown recurrence {value}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExpenseStatus {
    Planned,
    Active,
    Paid,
    Ended,
    Archived,
}

impl ExpenseStatus {
    pub(crate) fn as_db_value(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Active => "active",
            Self::Paid => "paid",
            Self::Ended => "ended",
            Self::Archived => "archived",
        }
    }

    pub(crate) fn parse(value: &str) -> AppResult<Self> {
        match value {
            "planned" => Ok(Self::Planned),
            "active" => Ok(Self::Active),
            "paid" => Ok(Self::Paid),
            "ended" => Ok(Self::Ended),
            "archived" => Ok(Self::Archived),
            _ => Err(AppError::Internal(format!(
                "unknown expense status {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceivableStatus {
    Owed,
    PartiallyPaid,
    Paid,
    Void,
    WrittenOff,
}

impl ReceivableStatus {
    pub(crate) fn as_db_value(self) -> &'static str {
        match self {
            Self::Owed => "owed",
            Self::PartiallyPaid => "partially_paid",
            Self::Paid => "paid",
            Self::Void => "void",
            Self::WrittenOff => "written_off",
        }
    }

    pub(crate) fn parse(value: &str) -> AppResult<Self> {
        match value {
            "owed" => Ok(Self::Owed),
            "partially_paid" => Ok(Self::PartiallyPaid),
            "paid" => Ok(Self::Paid),
            "void" => Ok(Self::Void),
            "written_off" => Ok(Self::WrittenOff),
            _ => Err(AppError::Internal(format!(
                "unknown receivable status {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinanceExpense {
    pub id: String,
    pub title: String,
    pub vendor_name: String,
    pub category: String,
    pub expense_kind: ExpenseKind,
    pub recurrence_interval: RecurrenceInterval,
    pub status: ExpenseStatus,
    pub amount_cents: i64,
    pub business_amount_cents: i64,
    pub personal_amount_cents: i64,
    pub currency: String,
    pub incurred_on: String,
    pub service_period_start: Option<String>,
    pub service_period_end: Option<String>,
    pub business_use_percent_bps: i32,
    pub source_message_id: Option<String>,
    pub source_attachment_id: Option<String>,
    pub source_asset_id: Option<String>,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinanceReceivable {
    pub id: String,
    pub contact_id: Option<String>,
    pub title: String,
    pub status: ReceivableStatus,
    pub amount_cents: i64,
    pub currency: String,
    pub issued_on: Option<String>,
    pub due_on: Option<String>,
    pub paid_on: Option<String>,
    pub source_message_id: Option<String>,
    pub source_booking_id: Option<String>,
    pub source_asset_id: Option<String>,
    pub external_reference: String,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinanceSummary {
    pub tax_year: i32,
    pub gross_expense_cents: i64,
    pub business_expense_cents: i64,
    pub personal_expense_cents: i64,
    pub receivable_owed_cents: i64,
    pub receivable_paid_cents: i64,
    pub category_totals: Vec<FinanceCategoryTotal>,
    pub vendor_totals: Vec<FinanceVendorTotal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinanceCategoryTotal {
    pub category: String,
    pub gross_cents: i64,
    pub business_cents: i64,
    pub personal_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinanceVendorTotal {
    pub vendor_name: String,
    pub gross_cents: i64,
    pub business_cents: i64,
    pub personal_cents: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FinanceExpenseQuery {
    pub tax_year: Option<i32>,
    pub status: Option<ExpenseStatus>,
    pub category: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FinanceReceivableQuery {
    pub contact_id: Option<String>,
    pub status: Option<ReceivableStatus>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FinanceSummaryQuery {
    pub tax_year: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateFinanceExpenseRequest {
    pub title: String,
    pub category: String,
    pub amount_cents: i64,
    pub incurred_on: String,
    pub vendor_name: Option<String>,
    pub expense_kind: Option<ExpenseKind>,
    pub recurrence_interval: Option<RecurrenceInterval>,
    pub status: Option<ExpenseStatus>,
    pub currency: Option<String>,
    pub service_period_start: Option<String>,
    pub service_period_end: Option<String>,
    pub business_use_percent_bps: Option<i32>,
    pub source_message_id: Option<String>,
    pub source_attachment_id: Option<String>,
    pub source_asset_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateFinanceExpenseRequest {
    pub title: Option<String>,
    pub vendor_name: Option<String>,
    pub category: Option<String>,
    pub expense_kind: Option<ExpenseKind>,
    pub recurrence_interval: Option<RecurrenceInterval>,
    pub status: Option<ExpenseStatus>,
    pub amount_cents: Option<i64>,
    pub currency: Option<String>,
    pub incurred_on: Option<String>,
    pub service_period_start: Option<String>,
    pub service_period_end: Option<String>,
    pub business_use_percent_bps: Option<i32>,
    pub source_message_id: Option<String>,
    pub source_attachment_id: Option<String>,
    pub source_asset_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateFinanceReceivableRequest {
    pub title: String,
    pub amount_cents: i64,
    pub contact_id: Option<String>,
    pub status: Option<ReceivableStatus>,
    pub currency: Option<String>,
    pub issued_on: Option<String>,
    pub due_on: Option<String>,
    pub paid_on: Option<String>,
    pub source_message_id: Option<String>,
    pub source_booking_id: Option<String>,
    pub source_asset_id: Option<String>,
    pub external_reference: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateFinanceReceivableRequest {
    pub contact_id: Option<String>,
    pub title: Option<String>,
    pub status: Option<ReceivableStatus>,
    pub amount_cents: Option<i64>,
    pub currency: Option<String>,
    pub issued_on: Option<String>,
    pub due_on: Option<String>,
    pub paid_on: Option<String>,
    pub source_message_id: Option<String>,
    pub source_booking_id: Option<String>,
    pub source_asset_id: Option<String>,
    pub external_reference: Option<String>,
    pub notes: Option<String>,
}
