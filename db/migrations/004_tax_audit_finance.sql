CREATE TABLE finance_expenses (
    id                       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title                    TEXT NOT NULL,
    vendor_name              TEXT NOT NULL DEFAULT '',
    category                 TEXT NOT NULL,
    expense_kind             TEXT NOT NULL DEFAULT 'one_time',
    recurrence_interval      TEXT NOT NULL DEFAULT 'none',
    status                   TEXT NOT NULL DEFAULT 'active',
    amount_cents             BIGINT NOT NULL,
    currency                 TEXT NOT NULL DEFAULT 'USD',
    incurred_on              DATE NOT NULL,
    service_period_start     DATE,
    service_period_end       DATE,
    business_use_percent_bps INTEGER NOT NULL DEFAULT 10000,
    source_message_id        UUID REFERENCES messages(id) ON DELETE SET NULL,
    source_attachment_id     UUID REFERENCES attachment_refs(id) ON DELETE SET NULL,
    source_asset_id          TEXT,
    notes                    TEXT NOT NULL DEFAULT '',
    created_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT finance_expenses_title_not_empty CHECK (title <> ''),
    CONSTRAINT finance_expenses_category_not_empty CHECK (category <> ''),
    CONSTRAINT finance_expenses_kind_valid
        CHECK (expense_kind IN ('one_time', 'recurring')),
    CONSTRAINT finance_expenses_recurrence_valid
        CHECK (recurrence_interval IN ('none', 'weekly', 'monthly', 'annual')),
    CONSTRAINT finance_expenses_status_valid
        CHECK (status IN ('planned', 'active', 'paid', 'ended', 'archived')),
    CONSTRAINT finance_expenses_amount_positive CHECK (amount_cents > 0),
    CONSTRAINT finance_expenses_currency_valid
        CHECK (currency = upper(currency) AND currency ~ '^[A-Z]{3}$'),
    CONSTRAINT finance_expenses_business_percent_valid
        CHECK (business_use_percent_bps BETWEEN 0 AND 10000),
    CONSTRAINT finance_expenses_period_order
        CHECK (
            service_period_start IS NULL
            OR service_period_end IS NULL
            OR service_period_end >= service_period_start
        )
);

CREATE INDEX idx_finance_expenses_incurred_on
    ON finance_expenses (incurred_on DESC, created_at DESC);

CREATE INDEX idx_finance_expenses_category
    ON finance_expenses (category, incurred_on DESC);

CREATE INDEX idx_finance_expenses_source_message
    ON finance_expenses (source_message_id)
    WHERE source_message_id IS NOT NULL;

CREATE TABLE finance_expense_audit (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    expense_id  UUID NOT NULL REFERENCES finance_expenses(id) ON DELETE CASCADE,
    action      TEXT NOT NULL,
    snapshot    JSONB NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT finance_expense_audit_action_valid
        CHECK (action IN ('created', 'updated'))
);

CREATE INDEX idx_finance_expense_audit_expense
    ON finance_expense_audit (expense_id, created_at DESC);

CREATE FUNCTION finance_expense_audit_row() RETURNS trigger AS $$
BEGIN
    INSERT INTO finance_expense_audit (expense_id, action, snapshot)
    VALUES (
        NEW.id,
        CASE WHEN TG_OP = 'INSERT' THEN 'created' ELSE 'updated' END,
        to_jsonb(NEW)
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_finance_expense_audit_row
    AFTER INSERT OR UPDATE ON finance_expenses
    FOR EACH ROW EXECUTE FUNCTION finance_expense_audit_row();

CREATE TABLE finance_receivables (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    contact_id         UUID REFERENCES contacts(id) ON DELETE SET NULL,
    title              TEXT NOT NULL,
    status             TEXT NOT NULL DEFAULT 'owed',
    amount_cents       BIGINT NOT NULL,
    currency           TEXT NOT NULL DEFAULT 'USD',
    issued_on          DATE,
    due_on             DATE,
    paid_on            DATE,
    source_message_id  UUID REFERENCES messages(id) ON DELETE SET NULL,
    source_booking_id  UUID REFERENCES bookings(id) ON DELETE SET NULL,
    source_asset_id    TEXT,
    external_reference TEXT NOT NULL DEFAULT '',
    notes              TEXT NOT NULL DEFAULT '',
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT finance_receivables_title_not_empty CHECK (title <> ''),
    CONSTRAINT finance_receivables_status_valid
        CHECK (status IN ('owed', 'partially_paid', 'paid', 'void', 'written_off')),
    CONSTRAINT finance_receivables_amount_positive CHECK (amount_cents > 0),
    CONSTRAINT finance_receivables_currency_valid
        CHECK (currency = upper(currency) AND currency ~ '^[A-Z]{3}$'),
    CONSTRAINT finance_receivables_due_after_issued
        CHECK (issued_on IS NULL OR due_on IS NULL OR due_on >= issued_on),
    CONSTRAINT finance_receivables_paid_date_required
        CHECK (status <> 'paid' OR paid_on IS NOT NULL)
);

CREATE INDEX idx_finance_receivables_status_due
    ON finance_receivables (status, due_on);

CREATE INDEX idx_finance_receivables_contact
    ON finance_receivables (contact_id, due_on)
    WHERE contact_id IS NOT NULL;

CREATE TABLE finance_receivable_audit (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    receivable_id UUID NOT NULL REFERENCES finance_receivables(id) ON DELETE CASCADE,
    action        TEXT NOT NULL,
    snapshot      JSONB NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT finance_receivable_audit_action_valid
        CHECK (action IN ('created', 'updated'))
);

CREATE INDEX idx_finance_receivable_audit_receivable
    ON finance_receivable_audit (receivable_id, created_at DESC);

CREATE FUNCTION finance_receivable_audit_row() RETURNS trigger AS $$
BEGIN
    INSERT INTO finance_receivable_audit (receivable_id, action, snapshot)
    VALUES (
        NEW.id,
        CASE WHEN TG_OP = 'INSERT' THEN 'created' ELSE 'updated' END,
        to_jsonb(NEW)
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_finance_receivable_audit_row
    AFTER INSERT OR UPDATE ON finance_receivables
    FOR EACH ROW EXECUTE FUNCTION finance_receivable_audit_row();
