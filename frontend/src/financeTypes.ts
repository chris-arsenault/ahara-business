export type ExpenseKind = "one_time" | "recurring";
export type RecurrenceInterval = "none" | "weekly" | "monthly" | "annual";
export type ExpenseStatus =
  | "planned"
  | "active"
  | "paid"
  | "ended"
  | "archived";
export type ReceivableStatus =
  | "owed"
  | "partially_paid"
  | "paid"
  | "void"
  | "written_off";

export type FinanceExpense = {
  id: string;
  title: string;
  vendor_name: string;
  category: string;
  expense_kind: ExpenseKind;
  recurrence_interval: RecurrenceInterval;
  status: ExpenseStatus;
  amount_cents: number;
  business_amount_cents: number;
  personal_amount_cents: number;
  currency: string;
  incurred_on: string;
  service_period_start: string | null;
  service_period_end: string | null;
  business_use_percent_bps: number;
  source_message_id: string | null;
  source_attachment_id: string | null;
  source_asset_id: string | null;
  notes: string;
  created_at: string;
  updated_at: string;
};

export type FinanceReceivable = {
  id: string;
  contact_id: string | null;
  title: string;
  status: ReceivableStatus;
  amount_cents: number;
  currency: string;
  issued_on: string | null;
  due_on: string | null;
  paid_on: string | null;
  source_message_id: string | null;
  source_booking_id: string | null;
  source_asset_id: string | null;
  external_reference: string;
  notes: string;
  created_at: string;
  updated_at: string;
};

export type FinanceSummary = {
  tax_year: number;
  gross_expense_cents: number;
  business_expense_cents: number;
  personal_expense_cents: number;
  receivable_owed_cents: number;
  receivable_paid_cents: number;
  category_totals: FinanceCategoryTotal[];
  vendor_totals: FinanceVendorTotal[];
};

export type FinanceCategoryTotal = {
  category: string;
  gross_cents: number;
  business_cents: number;
  personal_cents: number;
};

export type FinanceVendorTotal = {
  vendor_name: string;
  gross_cents: number;
  business_cents: number;
  personal_cents: number;
};

export type FinanceExpenseQuery = Partial<{
  tax_year: number;
  status: ExpenseStatus;
  category: string;
  limit: number;
}>;

export type FinanceReceivableQuery = Partial<{
  contact_id: string;
  status: ReceivableStatus;
  limit: number;
}>;

export type CreateFinanceExpenseRequest = {
  title: string;
  category: string;
  amount_cents: number;
  incurred_on: string;
} & Partial<{
  vendor_name: string | null;
  expense_kind: ExpenseKind;
  recurrence_interval: RecurrenceInterval;
  status: ExpenseStatus;
  currency: string | null;
  service_period_start: string | null;
  service_period_end: string | null;
  business_use_percent_bps: number;
  source_message_id: string | null;
  source_attachment_id: string | null;
  source_asset_id: string | null;
  notes: string | null;
}>;

export type UpdateFinanceExpenseRequest = Partial<
  Omit<
    CreateFinanceExpenseRequest,
    "title" | "category" | "amount_cents" | "incurred_on"
  > & {
    title: string;
    category: string;
    amount_cents: number;
    incurred_on: string;
  }
>;

export type CreateFinanceReceivableRequest = {
  title: string;
  amount_cents: number;
} & Partial<{
  contact_id: string | null;
  status: ReceivableStatus;
  currency: string | null;
  issued_on: string | null;
  due_on: string | null;
  paid_on: string | null;
  source_message_id: string | null;
  source_booking_id: string | null;
  source_asset_id: string | null;
  external_reference: string | null;
  notes: string | null;
}>;

export type UpdateFinanceReceivableRequest =
  Partial<CreateFinanceReceivableRequest>;
