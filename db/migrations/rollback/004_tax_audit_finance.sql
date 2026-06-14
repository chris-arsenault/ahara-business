DROP TRIGGER IF EXISTS trg_finance_receivable_audit_row ON finance_receivables;
DROP TRIGGER IF EXISTS trg_finance_expense_audit_row ON finance_expenses;
DROP FUNCTION IF EXISTS finance_receivable_audit_row();
DROP FUNCTION IF EXISTS finance_expense_audit_row();
DROP TABLE IF EXISTS finance_receivable_audit;
DROP TABLE IF EXISTS finance_receivables;
DROP TABLE IF EXISTS finance_expense_audit;
DROP TABLE IF EXISTS finance_expenses;
