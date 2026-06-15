use crate::error::{AppError, AppResult};
use crate::finance_model::{
    FinanceExpenseRow, NormalizedExpenseInput, optional_date, optional_text, validate_amount,
    validate_bps, validate_date,
};
use crate::finance_types::{
    CreateFinanceExpenseOccurrenceRequest, ExpenseKind, ExpenseStatus, RecurrenceInterval,
};

impl NormalizedExpenseInput {
    pub(crate) fn occurrence(
        parent: FinanceExpenseRow,
        request: CreateFinanceExpenseOccurrenceRequest,
    ) -> AppResult<Self> {
        let expense_kind = ExpenseKind::parse(&parent.expense_kind)?;
        let recurrence_interval = RecurrenceInterval::parse(&parent.recurrence_interval)?;
        let parent_status = ExpenseStatus::parse(&parent.status)?;
        validate_occurrence_parent(expense_kind, recurrence_interval, parent_status)?;
        let incurred_on = validate_date(&request.incurred_on, "incurred_on")?;
        let input = Self {
            title: parent.title,
            vendor_name: parent.vendor_name,
            category: parent.category,
            expense_kind,
            recurrence_interval,
            recurrence_parent_expense_id: Some(
                parent.recurrence_parent_expense_id.unwrap_or(parent.id),
            ),
            recurrence_instance_on: Some(incurred_on.clone()),
            status: request.status.unwrap_or(ExpenseStatus::Paid),
            amount_cents: validate_amount(request.amount_cents)?,
            currency: parent.currency,
            incurred_on,
            service_period_start: optional_date(
                request.service_period_start,
                "service_period_start",
            )?,
            service_period_end: optional_date(request.service_period_end, "service_period_end")?,
            business_use_percent_bps: validate_bps(
                request
                    .business_use_percent_bps
                    .unwrap_or(parent.business_use_percent_bps),
            )?,
            source_message_id: None,
            source_attachment_id: None,
            source_asset_id: None,
            notes: request
                .notes
                .and_then(optional_text)
                .unwrap_or(parent.notes),
        };
        input.validate_period()?;
        Ok(input)
    }
}

fn validate_occurrence_parent(
    kind: ExpenseKind,
    interval: RecurrenceInterval,
    status: ExpenseStatus,
) -> AppResult<()> {
    if kind != ExpenseKind::Recurring || interval == RecurrenceInterval::None {
        return Err(AppError::Validation(
            "expense occurrences require a recurring parent expense".to_string(),
        ));
    }
    if matches!(status, ExpenseStatus::Ended | ExpenseStatus::Archived) {
        return Err(AppError::Validation(
            "ended or archived expenses cannot create new occurrences".to_string(),
        ));
    }
    Ok(())
}
