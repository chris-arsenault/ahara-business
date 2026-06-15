DROP INDEX IF EXISTS idx_finance_expenses_recurrence_parent;

ALTER TABLE finance_expenses
    DROP CONSTRAINT IF EXISTS finance_expenses_recurrence_instance_not_self,
    DROP CONSTRAINT IF EXISTS finance_expenses_recurrence_instance_pair,
    DROP COLUMN IF EXISTS recurrence_instance_on,
    DROP COLUMN IF EXISTS recurrence_parent_expense_id;
