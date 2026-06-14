import type {
  ExpenseKind,
  ExpenseStatus,
  RecurrenceInterval,
  ReceivableStatus,
} from "./financeTypes";

export type ExpenseDraft = {
  title: string;
  vendor_name: string;
  category: string;
  amount: string;
  incurred_on: string;
  business_percent: string;
  expense_kind: ExpenseKind;
  recurrence_interval: RecurrenceInterval;
  status: ExpenseStatus;
  notes: string;
};

export type ReceivableDraft = {
  title: string;
  contact_id: string;
  amount: string;
  due_on: string;
  status: ReceivableStatus;
  external_reference: string;
  notes: string;
};

export function defaultExpenseDraft(): ExpenseDraft {
  return {
    title: "",
    vendor_name: "",
    category: "cloud",
    amount: "",
    incurred_on: today(),
    business_percent: "100",
    expense_kind: "recurring",
    recurrence_interval: "monthly",
    status: "active",
    notes: "",
  };
}

export function defaultReceivableDraft(): ReceivableDraft {
  return {
    title: "",
    contact_id: "",
    amount: "",
    due_on: today(),
    status: "owed",
    external_reference: "",
    notes: "",
  };
}

export function dollarsToCents(value: string) {
  const normalized = value.trim().replace(/[$,]/g, "");
  if (!normalized) {
    return 0;
  }
  return Math.round(Number(normalized) * 100);
}

export function percentToBps(value: string) {
  const normalized = value.trim().replace("%", "");
  if (!normalized) {
    return 0;
  }
  return Math.round(Number(normalized) * 100);
}

export function formatMoney(cents: number) {
  return new Intl.NumberFormat("en-US", {
    currency: "USD",
    style: "currency",
  }).format(cents / 100);
}

export function formatPercent(bps: number) {
  return `${(bps / 100).toFixed(bps % 100 === 0 ? 0 : 2)}%`;
}

function today() {
  return new Date().toISOString().slice(0, 10);
}
