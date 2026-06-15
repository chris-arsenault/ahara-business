pub(crate) const EXPENSE_SELECT_BY_ID: &str =
    "SELECT id, title, vendor_name, category, expense_kind,
    recurrence_interval, recurrence_parent_expense_id,
    recurrence_instance_on::text AS recurrence_instance_on,
    status, amount_cents, currency, incurred_on::text AS incurred_on,
    service_period_start::text AS service_period_start,
    service_period_end::text AS service_period_end, business_use_percent_bps,
    source_message_id, source_attachment_id, source_asset_id, notes,
    created_at::text AS created_at, updated_at::text AS updated_at
    FROM finance_expenses WHERE id = $1";

pub(crate) const EXPENSE_LIST: &str = "SELECT id, title, vendor_name, category, expense_kind,
    recurrence_interval, recurrence_parent_expense_id,
    recurrence_instance_on::text AS recurrence_instance_on,
    status, amount_cents, currency, incurred_on::text AS incurred_on,
    service_period_start::text AS service_period_start,
    service_period_end::text AS service_period_end, business_use_percent_bps,
    source_message_id, source_attachment_id, source_asset_id, notes,
    created_at::text AS created_at, updated_at::text AS updated_at
    FROM finance_expenses
    WHERE ($1::integer IS NULL OR EXTRACT(YEAR FROM incurred_on)::integer = $1)
      AND ($2::text IS NULL OR status = $2)
      AND ($3::text IS NULL OR category = $3)
    ORDER BY incurred_on DESC, created_at DESC LIMIT $4";

pub(crate) const EXPENSE_INSERT: &str = "INSERT INTO finance_expenses (
    title, vendor_name, category, expense_kind, recurrence_interval, status,
    amount_cents, currency, incurred_on, service_period_start, service_period_end,
    business_use_percent_bps, source_message_id, source_attachment_id,
    source_asset_id, notes, recurrence_parent_expense_id, recurrence_instance_on
) VALUES (
    $1, $2, $3, $4, $5, $6, $7, $8, $9::date, $10::date, $11::date,
    $12, $13, $14, $15, $16, $17, $18::date
) RETURNING id, title, vendor_name, category, expense_kind, recurrence_interval, status,
    recurrence_parent_expense_id, recurrence_instance_on::text AS recurrence_instance_on,
    amount_cents, currency, incurred_on::text AS incurred_on,
    service_period_start::text AS service_period_start,
    service_period_end::text AS service_period_end, business_use_percent_bps,
    source_message_id, source_attachment_id, source_asset_id, notes,
    created_at::text AS created_at, updated_at::text AS updated_at";

pub(crate) const EXPENSE_UPDATE: &str = "UPDATE finance_expenses
    SET title = $2, vendor_name = $3, category = $4, expense_kind = $5,
        recurrence_interval = $6, status = $7, amount_cents = $8,
        currency = $9, incurred_on = $10::date, service_period_start = $11::date,
        service_period_end = $12::date, business_use_percent_bps = $13,
        source_message_id = $14, source_attachment_id = $15, source_asset_id = $16,
        notes = $17, recurrence_parent_expense_id = $18,
        recurrence_instance_on = $19::date, updated_at = now()
    WHERE id = $1
    RETURNING id, title, vendor_name, category, expense_kind, recurrence_interval, status,
        recurrence_parent_expense_id, recurrence_instance_on::text AS recurrence_instance_on,
        amount_cents, currency, incurred_on::text AS incurred_on,
        service_period_start::text AS service_period_start,
        service_period_end::text AS service_period_end, business_use_percent_bps,
        source_message_id, source_attachment_id, source_asset_id, notes,
        created_at::text AS created_at, updated_at::text AS updated_at";

pub(crate) const RECEIVABLE_SELECT_BY_ID: &str = "SELECT id, contact_id, title, status,
    amount_cents, currency, issued_on::text AS issued_on, due_on::text AS due_on,
    paid_on::text AS paid_on, source_message_id, source_booking_id, source_asset_id,
    external_reference, notes, created_at::text AS created_at, updated_at::text AS updated_at
    FROM finance_receivables WHERE id = $1";

pub(crate) const RECEIVABLE_LIST: &str = "SELECT id, contact_id, title, status, amount_cents,
    currency, issued_on::text AS issued_on, due_on::text AS due_on,
    paid_on::text AS paid_on, source_message_id, source_booking_id, source_asset_id,
    external_reference, notes, created_at::text AS created_at, updated_at::text AS updated_at
    FROM finance_receivables
    WHERE ($1::uuid IS NULL OR contact_id = $1)
      AND ($2::text IS NULL OR status = $2)
    ORDER BY due_on ASC NULLS LAST, created_at DESC LIMIT $3";

pub(crate) const RECEIVABLE_INSERT: &str = "INSERT INTO finance_receivables (
    contact_id, title, status, amount_cents, currency, issued_on, due_on, paid_on,
    source_message_id, source_booking_id, source_asset_id, external_reference, notes
) VALUES (
    $1, $2, $3, $4, $5, $6::date, $7::date, $8::date, $9, $10, $11, $12, $13
) RETURNING id, contact_id, title, status, amount_cents, currency,
    issued_on::text AS issued_on, due_on::text AS due_on, paid_on::text AS paid_on,
    source_message_id, source_booking_id, source_asset_id, external_reference, notes,
    created_at::text AS created_at, updated_at::text AS updated_at";

pub(crate) const RECEIVABLE_UPDATE: &str = "UPDATE finance_receivables
    SET contact_id = $2, title = $3, status = $4, amount_cents = $5,
        currency = $6, issued_on = $7::date, due_on = $8::date,
        paid_on = $9::date, source_message_id = $10, source_booking_id = $11,
        source_asset_id = $12, external_reference = $13, notes = $14,
        updated_at = now()
    WHERE id = $1
    RETURNING id, contact_id, title, status, amount_cents, currency,
        issued_on::text AS issued_on, due_on::text AS due_on, paid_on::text AS paid_on,
        source_message_id, source_booking_id, source_asset_id, external_reference, notes,
        created_at::text AS created_at, updated_at::text AS updated_at";

pub(crate) const EXPENSE_TOTALS: &str = "SELECT
    COALESCE(SUM(amount_cents), 0)::bigint AS gross_cents,
    COALESCE(SUM((amount_cents * business_use_percent_bps) / 10000), 0)::bigint AS business_cents,
    COALESCE(SUM(amount_cents - ((amount_cents * business_use_percent_bps) / 10000)), 0)::bigint
        AS personal_cents
    FROM finance_expenses
    WHERE status <> 'archived'
      AND EXTRACT(YEAR FROM incurred_on)::integer = $1";

pub(crate) const RECEIVABLE_TOTALS: &str = "SELECT
    COALESCE(SUM(amount_cents) FILTER (WHERE status IN ('owed', 'partially_paid')), 0)::bigint
        AS owed_cents,
    COALESCE(SUM(amount_cents) FILTER (WHERE status = 'paid'), 0)::bigint AS paid_cents
    FROM finance_receivables";

pub(crate) const CATEGORY_TOTALS: &str = "SELECT category,
    COALESCE(SUM(amount_cents), 0)::bigint AS gross_cents,
    COALESCE(SUM((amount_cents * business_use_percent_bps) / 10000), 0)::bigint AS business_cents,
    COALESCE(SUM(amount_cents - ((amount_cents * business_use_percent_bps) / 10000)), 0)::bigint
        AS personal_cents
    FROM finance_expenses
    WHERE status <> 'archived'
      AND EXTRACT(YEAR FROM incurred_on)::integer = $1
    GROUP BY category
    ORDER BY business_cents DESC, category ASC";

pub(crate) const VENDOR_TOTALS: &str =
    "SELECT COALESCE(NULLIF(vendor_name, ''), '(none)') AS vendor_name,
    COALESCE(SUM(amount_cents), 0)::bigint AS gross_cents,
    COALESCE(SUM((amount_cents * business_use_percent_bps) / 10000), 0)::bigint AS business_cents,
    COALESCE(SUM(amount_cents - ((amount_cents * business_use_percent_bps) / 10000)), 0)::bigint
        AS personal_cents
    FROM finance_expenses
    WHERE status <> 'archived'
      AND EXTRACT(YEAR FROM incurred_on)::integer = $1
    GROUP BY COALESCE(NULLIF(vendor_name, ''), '(none)')
    ORDER BY business_cents DESC, vendor_name ASC";
