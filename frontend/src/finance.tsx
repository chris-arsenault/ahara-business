/* eslint-disable max-lines-per-function, react-hooks/exhaustive-deps, react-hooks/set-state-in-effect, react-perf/jsx-no-new-function-as-prop */
import { useEffect, useState, type FormEvent } from "react";
import { Plus } from "lucide-react";
import type { ApiClient } from "./api";
import type { FinanceApiSurface } from "./financeApi";
import {
  defaultOccurrenceDraft,
  defaultExpenseDraft,
  defaultReceivableDraft,
  dollarsToCents,
  percentToBps,
  type ExpenseOccurrenceDraft,
  type ExpenseDraft,
  type ReceivableDraft,
} from "./financeDrafts";
import {
  ContactSelect,
  DateInput,
  ExpenseList,
  FinanceShell,
  ReceivableList,
  SelectInput,
  SummaryCards,
  TextArea,
  TextInput,
} from "./financeParts";
import {
  expenseKinds,
  expenseStatuses,
  receivableStatuses,
  recurrenceIntervals,
} from "./financeOptions";
import type {
  ExpenseKind,
  ExpenseStatus,
  FinanceExpense,
  FinanceReceivable,
  FinanceSummary,
  ReceivableStatus,
  RecurrenceInterval,
} from "./financeTypes";
import type { Contact } from "./types";

export type FinanceApi = FinanceApiSurface & Pick<ApiClient, "listContacts">;

type State =
  | { status: "loading" }
  | {
      status: "ready";
      expenses: FinanceExpense[];
      receivables: FinanceReceivable[];
      summary: FinanceSummary;
      contacts: Contact[];
    }
  | { status: "error"; message: string };

export function FinanceView({ apiClient }: { apiClient: FinanceApi }) {
  const [taxYear, setTaxYear] = useState(String(new Date().getFullYear()));
  const [state, setState] = useState<State>({ status: "loading" });
  const [expenseDraft, setExpenseDraft] = useState<ExpenseDraft>(() =>
    defaultExpenseDraft(),
  );
  const [occurrenceDrafts, setOccurrenceDrafts] = useState<
    Record<string, ExpenseOccurrenceDraft>
  >({});
  const [receivableDraft, setReceivableDraft] = useState<ReceivableDraft>(() =>
    defaultReceivableDraft(),
  );
  const [actionError, setActionError] = useState<string>();

  async function load() {
    setState({ status: "loading" });
    try {
      const year = Number(taxYear);
      const [expenses, receivables, summary, contacts] = await Promise.all([
        apiClient.listFinanceExpenses({ limit: 250, tax_year: year }),
        apiClient.listFinanceReceivables({ limit: 250 }),
        apiClient.getFinanceSummary(year),
        apiClient.listContacts(),
      ]);
      setState({ status: "ready", expenses, receivables, summary, contacts });
    } catch (error) {
      setState({
        status: "error",
        message:
          error instanceof Error ? error.message : "Unable to load finance",
      });
    }
  }

  useEffect(() => {
    void load();
  }, [apiClient, taxYear]);

  if (state.status === "loading") {
    return (
      <FinanceShell
        body={<div className="empty-state">Loading finance</div>}
        onRefresh={null}
      />
    );
  }
  if (state.status === "error") {
    return (
      <FinanceShell
        body={<div className="error-state">{state.message}</div>}
        onRefresh={null}
      />
    );
  }

  const contactName = (id: string | null) =>
    state.contacts.find((contact) => contact.id === id)?.display_name ?? "";

  return (
    <FinanceShell
      body={
        <>
          {actionError ? (
            <div className="error-state compact-error" role="alert">
              {actionError}
            </div>
          ) : null}
          <div className="business-grid finance-controls">
            <TextInput label="Tax year" value={taxYear} onChange={setTaxYear} />
          </div>
          <SummaryCards summary={state.summary} />
          <div className="business-grid">
            <ExpenseForm
              draft={expenseDraft}
              onPatch={patchExpense}
              onSubmit={createExpense}
            />
            <ReceivableForm
              contacts={state.contacts}
              draft={receivableDraft}
              onPatch={patchReceivable}
              onSubmit={createReceivable}
            />
          </div>
          <div className="business-grid wide">
            <ExpenseList
              expenses={state.expenses}
              occurrenceDrafts={occurrenceDrafts}
              onOccurrence={createExpenseOccurrence}
              onOccurrenceDraft={setOccurrenceDrafts}
              onStatus={updateExpenseStatus}
            />
            <ReceivableList
              receivables={state.receivables}
              contactName={contactName}
              onStatus={updateReceivableStatus}
            />
          </div>
        </>
      }
      onRefresh={() => void load()}
    />
  );

  function patchExpense(field: keyof ExpenseDraft) {
    return (value: string) =>
      setExpenseDraft(
        (current) => ({ ...current, [field]: value }) as ExpenseDraft,
      );
  }

  function patchReceivable(field: keyof ReceivableDraft) {
    return (value: string) =>
      setReceivableDraft(
        (current) => ({ ...current, [field]: value }) as ReceivableDraft,
      );
  }

  async function createExpense(event: FormEvent) {
    event.preventDefault();
    await runAction(async () => {
      await apiClient.createFinanceExpense({
        title: expenseDraft.title,
        vendor_name: expenseDraft.vendor_name || null,
        category: expenseDraft.category,
        expense_kind: expenseDraft.expense_kind,
        recurrence_interval: expenseDraft.recurrence_interval,
        status: expenseDraft.status,
        amount_cents: dollarsToCents(expenseDraft.amount),
        incurred_on: expenseDraft.incurred_on,
        business_use_percent_bps: percentToBps(expenseDraft.business_percent),
        notes: expenseDraft.notes || null,
      });
      setExpenseDraft(defaultExpenseDraft());
    });
  }

  async function createReceivable(event: FormEvent) {
    event.preventDefault();
    await runAction(async () => {
      await apiClient.createFinanceReceivable({
        title: receivableDraft.title,
        amount_cents: dollarsToCents(receivableDraft.amount),
        contact_id: receivableDraft.contact_id || null,
        due_on: receivableDraft.due_on || null,
        status: receivableDraft.status,
        paid_on: receivableDraft.status === "paid" ? todayDate() : null,
        external_reference: receivableDraft.external_reference || null,
        notes: receivableDraft.notes || null,
      });
      setReceivableDraft(defaultReceivableDraft());
    });
  }

  async function updateExpenseStatus(id: string, status: ExpenseStatus) {
    await runAction(() => apiClient.updateFinanceExpense(id, { status }));
  }

  async function createExpenseOccurrence(
    expense: FinanceExpense,
    draft: ExpenseOccurrenceDraft,
  ) {
    await runAction(async () => {
      await apiClient.createFinanceExpenseOccurrence(expense.id, {
        amount_cents: dollarsToCents(draft.amount),
        incurred_on: draft.incurred_on,
        status: "paid",
      });
      setOccurrenceDrafts((current) => ({
        ...current,
        [expense.id]: defaultOccurrenceDraft(expense),
      }));
    });
  }

  async function updateReceivableStatus(id: string, status: ReceivableStatus) {
    await runAction(() =>
      apiClient.updateFinanceReceivable(id, {
        paid_on: status === "paid" ? todayDate() : undefined,
        status,
      }),
    );
  }

  async function runAction(action: () => Promise<unknown>) {
    setActionError(undefined);
    try {
      await action();
      await load();
    } catch (error) {
      setActionError(error instanceof Error ? error.message : "Action failed");
    }
  }
}

function ExpenseForm({
  draft,
  onPatch,
  onSubmit,
}: {
  draft: ExpenseDraft;
  onPatch: (field: keyof ExpenseDraft) => (value: string) => void;
  onSubmit: (event: FormEvent) => void;
}) {
  return (
    <form className="business-form" onSubmit={onSubmit}>
      <h2>Expense</h2>
      <TextInput
        label="Title"
        value={draft.title}
        onChange={onPatch("title")}
      />
      <TextInput
        label="Vendor"
        value={draft.vendor_name}
        onChange={onPatch("vendor_name")}
      />
      <TextInput
        label="Category"
        value={draft.category}
        onChange={onPatch("category")}
      />
      <TextInput
        label="Amount"
        value={draft.amount}
        onChange={onPatch("amount")}
      />
      <DateInput
        label="Incurred"
        value={draft.incurred_on}
        onChange={onPatch("incurred_on")}
      />
      <TextInput
        label="Business %"
        value={draft.business_percent}
        onChange={onPatch("business_percent")}
      />
      <SelectInput
        label="Kind"
        value={draft.expense_kind}
        values={expenseKinds}
        onChange={(value) => onPatch("expense_kind")(value as ExpenseKind)}
      />
      <SelectInput
        label="Recurrence"
        value={draft.recurrence_interval}
        values={recurrenceIntervals}
        onChange={(value) =>
          onPatch("recurrence_interval")(value as RecurrenceInterval)
        }
      />
      <SelectInput
        label="Status"
        value={draft.status}
        values={expenseStatuses}
        onChange={(value) => onPatch("status")(value as ExpenseStatus)}
      />
      <TextArea label="Notes" value={draft.notes} onChange={onPatch("notes")} />
      <button className="secondary-button" type="submit">
        <Plus aria-hidden="true" size={15} />
        Add expense
      </button>
    </form>
  );
}

function ReceivableForm({
  contacts,
  draft,
  onPatch,
  onSubmit,
}: {
  contacts: Contact[];
  draft: ReceivableDraft;
  onPatch: (field: keyof ReceivableDraft) => (value: string) => void;
  onSubmit: (event: FormEvent) => void;
}) {
  return (
    <form className="business-form" onSubmit={onSubmit}>
      <h2>Client owed/paid</h2>
      <TextInput
        label="Title"
        value={draft.title}
        onChange={onPatch("title")}
      />
      <ContactSelect
        contacts={contacts}
        value={draft.contact_id}
        onChange={onPatch("contact_id")}
      />
      <TextInput
        label="Amount"
        value={draft.amount}
        onChange={onPatch("amount")}
      />
      <DateInput
        label="Due"
        value={draft.due_on}
        onChange={onPatch("due_on")}
      />
      <SelectInput
        label="Status"
        value={draft.status}
        values={receivableStatuses}
        onChange={(value) => onPatch("status")(value as ReceivableStatus)}
      />
      <TextInput
        label="Reference"
        value={draft.external_reference}
        onChange={onPatch("external_reference")}
      />
      <TextArea label="Notes" value={draft.notes} onChange={onPatch("notes")} />
      <button className="secondary-button" type="submit">
        <Plus aria-hidden="true" size={15} />
        Add receivable
      </button>
    </form>
  );
}

function todayDate() {
  return new Date().toISOString().slice(0, 10);
}
