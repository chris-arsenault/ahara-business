/* eslint-disable react-perf/jsx-no-new-function-as-prop */
import { ReceiptText, RefreshCw } from "lucide-react";
import type { ReactNode } from "react";
import type { Contact } from "./types";
import { formatMoney, formatPercent } from "./financeDrafts";
import { expenseStatuses, receivableStatuses } from "./financeOptions";
import type {
  ExpenseStatus,
  FinanceExpense,
  FinanceReceivable,
  FinanceSummary,
  ReceivableStatus,
} from "./financeTypes";

export function FinanceShell({
  body,
  onRefresh,
}: {
  body: ReactNode;
  onRefresh: (() => void) | null;
}) {
  return (
    <section className="admin-panel" aria-labelledby="finance-title">
      <header className="admin-toolbar">
        <div className="toolbar-title">
          <ReceiptText aria-hidden="true" size={18} />
          <h1 id="finance-title">Finance</h1>
        </div>
        {onRefresh ? (
          <button
            className="icon-button"
            type="button"
            title="Refresh"
            aria-label="Refresh finance"
            onClick={onRefresh}
          >
            <RefreshCw aria-hidden="true" size={16} />
          </button>
        ) : null}
      </header>
      {body}
    </section>
  );
}

export function TextInput({ label, value, onChange }: FieldProps) {
  return (
    <label className="field-control">
      <span>{label}</span>
      <input value={value} onChange={(e) => onChange(e.currentTarget.value)} />
    </label>
  );
}

export function DateInput({ label, value, onChange }: FieldProps) {
  return (
    <label className="field-control">
      <span>{label}</span>
      <input
        type="date"
        value={value}
        onChange={(e) => onChange(e.currentTarget.value)}
      />
    </label>
  );
}

export function TextArea({ label, value, onChange }: FieldProps) {
  return (
    <label className="field-control">
      <span>{label}</span>
      <textarea
        value={value}
        onChange={(e) => onChange(e.currentTarget.value)}
      />
    </label>
  );
}

export function SelectInput({
  label,
  value,
  values,
  onChange,
}: FieldProps & { values: string[] }) {
  return (
    <label className="field-control">
      <span>{label}</span>
      <select value={value} onChange={(e) => onChange(e.currentTarget.value)}>
        {values.map((item) => (
          <option key={item} value={item}>
            {item}
          </option>
        ))}
      </select>
    </label>
  );
}

export function ContactSelect({
  contacts,
  value,
  onChange,
}: {
  contacts: Contact[];
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="field-control">
      <span>Contact</span>
      <select value={value} onChange={(e) => onChange(e.currentTarget.value)}>
        <option value="">None</option>
        {contacts.map((contact) => (
          <option key={contact.id} value={contact.id}>
            {contact.display_name}
          </option>
        ))}
      </select>
    </label>
  );
}

export function SummaryCards({ summary }: { summary: FinanceSummary }) {
  return (
    <section className="finance-summary-grid" aria-label="Finance summary">
      <SummaryCard label="Gross expenses" value={summary.gross_expense_cents} />
      <SummaryCard
        label="Business expense"
        value={summary.business_expense_cents}
      />
      <SummaryCard
        label="Personal expense"
        value={summary.personal_expense_cents}
      />
      <SummaryCard label="Client owed" value={summary.receivable_owed_cents} />
      <SummaryCard label="Client paid" value={summary.receivable_paid_cents} />
    </section>
  );
}

export function ExpenseList({
  expenses,
  onStatus,
}: {
  expenses: FinanceExpense[];
  onStatus: (id: string, status: ExpenseStatus) => void;
}) {
  return (
    <section className="business-list finance-list">
      <h2>Expenses</h2>
      {expenses.map((expense) => (
        <article key={expense.id}>
          <strong>{expense.title}</strong>
          <span>{expense.vendor_name || expense.category}</span>
          <small>
            {formatMoney(expense.business_amount_cents)} business /{" "}
            {formatPercent(expense.business_use_percent_bps)}
          </small>
          <StatusSelect
            value={expense.status}
            values={expenseStatuses}
            onChange={(status) => onStatus(expense.id, status as ExpenseStatus)}
          />
        </article>
      ))}
    </section>
  );
}

export function ReceivableList({
  receivables,
  contactName,
  onStatus,
}: {
  receivables: FinanceReceivable[];
  contactName: (id: string | null) => string;
  onStatus: (id: string, status: ReceivableStatus) => void;
}) {
  return (
    <section className="business-list finance-list">
      <h2>Client owed/paid</h2>
      {receivables.map((receivable) => (
        <article key={receivable.id}>
          <strong>{receivable.title}</strong>
          <span>{formatMoney(receivable.amount_cents)}</span>
          <small>{contactName(receivable.contact_id)}</small>
          <StatusSelect
            value={receivable.status}
            values={receivableStatuses}
            onChange={(status) =>
              onStatus(receivable.id, status as ReceivableStatus)
            }
          />
        </article>
      ))}
    </section>
  );
}

type FieldProps = {
  label: string;
  value: string;
  onChange: (value: string) => void;
};

function SummaryCard({ label, value }: { label: string; value: number }) {
  return (
    <div>
      <span>{label}</span>
      <strong>{formatMoney(value)}</strong>
    </div>
  );
}

function StatusSelect({
  value,
  values,
  onChange,
}: {
  value: string;
  values: string[];
  onChange: (value: string) => void;
}) {
  return (
    <select value={value} onChange={(e) => onChange(e.currentTarget.value)}>
      {values.map((item) => (
        <option key={item} value={item}>
          {item}
        </option>
      ))}
    </select>
  );
}
