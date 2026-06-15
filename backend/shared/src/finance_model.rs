use sqlx::FromRow;
use time::{Date, Month};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::finance_types::{
    CreateFinanceExpenseRequest, CreateFinanceReceivableRequest, ExpenseKind, ExpenseStatus,
    ReceivableStatus, RecurrenceInterval, UpdateFinanceExpenseRequest,
    UpdateFinanceReceivableRequest,
};

#[derive(Debug, Clone, FromRow)]
pub(crate) struct FinanceExpenseRow {
    pub(crate) id: Uuid,
    pub(crate) title: String,
    pub(crate) vendor_name: String,
    pub(crate) category: String,
    pub(crate) expense_kind: String,
    pub(crate) recurrence_interval: String,
    pub(crate) recurrence_parent_expense_id: Option<Uuid>,
    pub(crate) recurrence_instance_on: Option<String>,
    pub(crate) status: String,
    pub(crate) amount_cents: i64,
    pub(crate) currency: String,
    pub(crate) incurred_on: String,
    pub(crate) service_period_start: Option<String>,
    pub(crate) service_period_end: Option<String>,
    pub(crate) business_use_percent_bps: i32,
    pub(crate) source_message_id: Option<Uuid>,
    pub(crate) source_attachment_id: Option<Uuid>,
    pub(crate) source_asset_id: Option<String>,
    pub(crate) notes: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct FinanceReceivableRow {
    pub(crate) id: Uuid,
    pub(crate) contact_id: Option<Uuid>,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) amount_cents: i64,
    pub(crate) currency: String,
    pub(crate) issued_on: Option<String>,
    pub(crate) due_on: Option<String>,
    pub(crate) paid_on: Option<String>,
    pub(crate) source_message_id: Option<Uuid>,
    pub(crate) source_booking_id: Option<Uuid>,
    pub(crate) source_asset_id: Option<String>,
    pub(crate) external_reference: String,
    pub(crate) notes: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Debug, FromRow)]
pub(crate) struct FinanceCategoryTotalRow {
    pub(crate) category: String,
    pub(crate) gross_cents: i64,
    pub(crate) business_cents: i64,
    pub(crate) personal_cents: i64,
}

#[derive(Debug, FromRow)]
pub(crate) struct FinanceVendorTotalRow {
    pub(crate) vendor_name: String,
    pub(crate) gross_cents: i64,
    pub(crate) business_cents: i64,
    pub(crate) personal_cents: i64,
}

pub(crate) struct NormalizedExpenseInput {
    pub(crate) title: String,
    pub(crate) vendor_name: String,
    pub(crate) category: String,
    pub(crate) expense_kind: ExpenseKind,
    pub(crate) recurrence_interval: RecurrenceInterval,
    pub(crate) recurrence_parent_expense_id: Option<Uuid>,
    pub(crate) recurrence_instance_on: Option<String>,
    pub(crate) status: ExpenseStatus,
    pub(crate) amount_cents: i64,
    pub(crate) currency: String,
    pub(crate) incurred_on: String,
    pub(crate) service_period_start: Option<String>,
    pub(crate) service_period_end: Option<String>,
    pub(crate) business_use_percent_bps: i32,
    pub(crate) source_message_id: Option<Uuid>,
    pub(crate) source_attachment_id: Option<Uuid>,
    pub(crate) source_asset_id: Option<String>,
    pub(crate) notes: String,
}

impl NormalizedExpenseInput {
    pub(crate) fn create(request: CreateFinanceExpenseRequest) -> AppResult<Self> {
        let input = Self {
            title: required_text("title", request.title)?,
            vendor_name: request
                .vendor_name
                .and_then(optional_text)
                .unwrap_or_default(),
            category: required_text("category", request.category)?,
            expense_kind: request.expense_kind.unwrap_or(ExpenseKind::OneTime),
            recurrence_interval: request
                .recurrence_interval
                .unwrap_or(RecurrenceInterval::None),
            recurrence_parent_expense_id: None,
            recurrence_instance_on: None,
            status: request.status.unwrap_or(ExpenseStatus::Active),
            amount_cents: validate_amount(request.amount_cents)?,
            currency: validate_currency(request.currency.as_deref().unwrap_or("USD"))?,
            incurred_on: validate_date(&request.incurred_on, "incurred_on")?,
            service_period_start: optional_date(
                request.service_period_start,
                "service_period_start",
            )?,
            service_period_end: optional_date(request.service_period_end, "service_period_end")?,
            business_use_percent_bps: validate_bps(
                request.business_use_percent_bps.unwrap_or(10000),
            )?,
            source_message_id: parse_optional_uuid(
                request.source_message_id.as_deref(),
                "source message id",
            )?,
            source_attachment_id: parse_optional_uuid(
                request.source_attachment_id.as_deref(),
                "source attachment id",
            )?,
            source_asset_id: request.source_asset_id.and_then(optional_text),
            notes: request.notes.and_then(optional_text).unwrap_or_default(),
        };
        input.validate_period()?;
        Ok(input)
    }

    pub(crate) fn update(
        current: FinanceExpenseRow,
        request: UpdateFinanceExpenseRequest,
    ) -> AppResult<Self> {
        let input = Self {
            title: request
                .title
                .map_or(Ok(current.title), |v| required_text("title", v))?,
            vendor_name: request
                .vendor_name
                .and_then(optional_text)
                .unwrap_or(current.vendor_name),
            category: request
                .category
                .map_or(Ok(current.category), |v| required_text("category", v))?,
            expense_kind: request
                .expense_kind
                .unwrap_or(ExpenseKind::parse(&current.expense_kind)?),
            recurrence_interval: request
                .recurrence_interval
                .unwrap_or(RecurrenceInterval::parse(&current.recurrence_interval)?),
            recurrence_parent_expense_id: current.recurrence_parent_expense_id,
            recurrence_instance_on: current.recurrence_instance_on,
            status: request
                .status
                .unwrap_or(ExpenseStatus::parse(&current.status)?),
            amount_cents: validate_amount(request.amount_cents.unwrap_or(current.amount_cents))?,
            currency: validate_currency(request.currency.as_deref().unwrap_or(&current.currency))?,
            incurred_on: optional_date(request.incurred_on, "incurred_on")?
                .unwrap_or(current.incurred_on),
            service_period_start: optional_date(
                request.service_period_start,
                "service_period_start",
            )?
            .or(current.service_period_start),
            service_period_end: optional_date(request.service_period_end, "service_period_end")?
                .or(current.service_period_end),
            business_use_percent_bps: validate_bps(
                request
                    .business_use_percent_bps
                    .unwrap_or(current.business_use_percent_bps),
            )?,
            source_message_id: request
                .source_message_id
                .map(|id| parse_optional_uuid(Some(&id), "source message id"))
                .unwrap_or(Ok(current.source_message_id))?,
            source_attachment_id: request
                .source_attachment_id
                .map(|id| parse_optional_uuid(Some(&id), "source attachment id"))
                .unwrap_or(Ok(current.source_attachment_id))?,
            source_asset_id: request
                .source_asset_id
                .and_then(optional_text)
                .or(current.source_asset_id),
            notes: request
                .notes
                .and_then(optional_text)
                .unwrap_or(current.notes),
        };
        input.validate_period()?;
        Ok(input)
    }

    pub(crate) fn validate_period(&self) -> AppResult<()> {
        validate_date_order(
            self.service_period_start.as_deref(),
            self.service_period_end.as_deref(),
            "service_period_end must be on or after service_period_start",
        )
    }
}

pub(crate) struct NormalizedReceivableInput {
    pub(crate) contact_id: Option<Uuid>,
    pub(crate) title: String,
    pub(crate) status: ReceivableStatus,
    pub(crate) amount_cents: i64,
    pub(crate) currency: String,
    pub(crate) issued_on: Option<String>,
    pub(crate) due_on: Option<String>,
    pub(crate) paid_on: Option<String>,
    pub(crate) source_message_id: Option<Uuid>,
    pub(crate) source_booking_id: Option<Uuid>,
    pub(crate) source_asset_id: Option<String>,
    pub(crate) external_reference: String,
    pub(crate) notes: String,
}

impl NormalizedReceivableInput {
    pub(crate) fn create(request: CreateFinanceReceivableRequest) -> AppResult<Self> {
        let input = Self {
            contact_id: parse_optional_uuid(request.contact_id.as_deref(), "contact id")?,
            title: required_text("title", request.title)?,
            status: request.status.unwrap_or(ReceivableStatus::Owed),
            amount_cents: validate_amount(request.amount_cents)?,
            currency: validate_currency(request.currency.as_deref().unwrap_or("USD"))?,
            issued_on: optional_date(request.issued_on, "issued_on")?,
            due_on: optional_date(request.due_on, "due_on")?,
            paid_on: optional_date(request.paid_on, "paid_on")?,
            source_message_id: parse_optional_uuid(
                request.source_message_id.as_deref(),
                "source message id",
            )?,
            source_booking_id: parse_optional_uuid(
                request.source_booking_id.as_deref(),
                "source booking id",
            )?,
            source_asset_id: request.source_asset_id.and_then(optional_text),
            external_reference: request
                .external_reference
                .and_then(optional_text)
                .unwrap_or_default(),
            notes: request.notes.and_then(optional_text).unwrap_or_default(),
        };
        input.validate_dates()?;
        Ok(input)
    }

    pub(crate) fn update(
        current: FinanceReceivableRow,
        request: UpdateFinanceReceivableRequest,
    ) -> AppResult<Self> {
        let input = Self {
            contact_id: request
                .contact_id
                .map(|id| parse_optional_uuid(Some(&id), "contact id"))
                .unwrap_or(Ok(current.contact_id))?,
            title: request
                .title
                .map_or(Ok(current.title), |v| required_text("title", v))?,
            status: request
                .status
                .unwrap_or(ReceivableStatus::parse(&current.status)?),
            amount_cents: validate_amount(request.amount_cents.unwrap_or(current.amount_cents))?,
            currency: validate_currency(request.currency.as_deref().unwrap_or(&current.currency))?,
            issued_on: optional_date(request.issued_on, "issued_on")?.or(current.issued_on),
            due_on: optional_date(request.due_on, "due_on")?.or(current.due_on),
            paid_on: optional_date(request.paid_on, "paid_on")?.or(current.paid_on),
            source_message_id: request
                .source_message_id
                .map(|id| parse_optional_uuid(Some(&id), "source message id"))
                .unwrap_or(Ok(current.source_message_id))?,
            source_booking_id: request
                .source_booking_id
                .map(|id| parse_optional_uuid(Some(&id), "source booking id"))
                .unwrap_or(Ok(current.source_booking_id))?,
            source_asset_id: request
                .source_asset_id
                .and_then(optional_text)
                .or(current.source_asset_id),
            external_reference: request
                .external_reference
                .and_then(optional_text)
                .unwrap_or(current.external_reference),
            notes: request
                .notes
                .and_then(optional_text)
                .unwrap_or(current.notes),
        };
        input.validate_dates()?;
        Ok(input)
    }

    fn validate_dates(&self) -> AppResult<()> {
        validate_date_order(
            self.issued_on.as_deref(),
            self.due_on.as_deref(),
            "due_on must be on or after issued_on",
        )?;
        if self.status == ReceivableStatus::Paid && self.paid_on.is_none() {
            return Err(AppError::Validation(
                "paid_on is required when receivable is paid".to_string(),
            ));
        }
        Ok(())
    }
}

pub(crate) fn parse_uuid(value: &str, label: &str) -> AppResult<Uuid> {
    Uuid::parse_str(value).map_err(|_| AppError::Validation(format!("{label} must be a UUID")))
}

pub(crate) fn parse_optional_uuid(value: Option<&str>, label: &str) -> AppResult<Option<Uuid>> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| parse_uuid(value, label))
        .transpose()
}

pub(crate) fn required_text(label: &str, value: String) -> AppResult<String> {
    optional_text(value).ok_or_else(|| AppError::Validation(format!("{label} is required")))
}

pub(crate) fn optional_text(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

pub(crate) fn validate_amount(value: i64) -> AppResult<i64> {
    (value > 0)
        .then_some(value)
        .ok_or_else(|| AppError::Validation("amount_cents must be positive".to_string()))
}

pub(crate) fn validate_bps(value: i32) -> AppResult<i32> {
    (0..=10000)
        .contains(&value)
        .then_some(value)
        .ok_or_else(|| {
            AppError::Validation("business_use_percent_bps must be 0..10000".to_string())
        })
}

fn validate_currency(value: &str) -> AppResult<String> {
    let value = value.trim().to_ascii_uppercase();
    if value.len() == 3 && value.chars().all(|ch| ch.is_ascii_uppercase()) {
        return Ok(value);
    }
    Err(AppError::Validation(
        "currency must be a 3-letter code".to_string(),
    ))
}

pub(crate) fn optional_date(value: Option<String>, label: &str) -> AppResult<Option<String>> {
    value.map(|value| validate_date(&value, label)).transpose()
}

pub(crate) fn validate_date(value: &str, label: &str) -> AppResult<String> {
    parse_date(value, label)?;
    Ok(value.trim().to_string())
}

fn validate_date_order(start: Option<&str>, end: Option<&str>, message: &str) -> AppResult<()> {
    let (Some(start), Some(end)) = (start, end) else {
        return Ok(());
    };
    if parse_date(end, "end")? < parse_date(start, "start")? {
        return Err(AppError::Validation(message.to_string()));
    }
    Ok(())
}

fn parse_date(value: &str, label: &str) -> AppResult<Date> {
    let value = value.trim();
    if value.len() != 10 || !value.is_ascii() || &value[4..5] != "-" || &value[7..8] != "-" {
        return Err(AppError::Validation(format!("{label} must be YYYY-MM-DD")));
    }
    let year = value[0..4]
        .parse::<i32>()
        .map_err(|_| AppError::Validation(format!("{label} must be YYYY-MM-DD")))?;
    let month = value[5..7]
        .parse::<u8>()
        .ok()
        .and_then(|month| Month::try_from(month).ok())
        .ok_or_else(|| AppError::Validation(format!("{label} must be YYYY-MM-DD")))?;
    let day = value[8..10]
        .parse::<u8>()
        .map_err(|_| AppError::Validation(format!("{label} must be YYYY-MM-DD")))?;
    Date::from_calendar_date(year, month, day)
        .map_err(|_| AppError::Validation(format!("{label} must be YYYY-MM-DD")))
}
