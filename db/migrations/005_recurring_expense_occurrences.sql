ALTER TABLE finance_expenses
    ADD COLUMN recurrence_parent_expense_id UUID REFERENCES finance_expenses(id),
    ADD COLUMN recurrence_instance_on DATE;

ALTER TABLE finance_expenses
    ADD CONSTRAINT finance_expenses_recurrence_instance_pair
        CHECK (
            (recurrence_parent_expense_id IS NULL)
            = (recurrence_instance_on IS NULL)
        ),
    ADD CONSTRAINT finance_expenses_recurrence_instance_not_self
        CHECK (
            recurrence_parent_expense_id IS NULL
            OR recurrence_parent_expense_id <> id
        );

CREATE INDEX idx_finance_expenses_recurrence_parent
    ON finance_expenses (recurrence_parent_expense_id, recurrence_instance_on DESC)
    WHERE recurrence_parent_expense_id IS NOT NULL;
