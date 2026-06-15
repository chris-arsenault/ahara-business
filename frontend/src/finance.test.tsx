import "@testing-library/jest-dom/vitest";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FinanceView, type FinanceApi } from "./finance";
import type {
  FinanceExpense,
  FinanceReceivable,
  FinanceSummary,
} from "./financeTypes";
import type { Contact } from "./types";

const contact: Contact = {
  id: "contact-1",
  display_name: "Client",
  primary_address: "client@example.com",
  primary_address_normalized: "client@example.com",
  notes: "",
};

const expense: FinanceExpense = {
  id: "expense-1",
  title: "Cloud hosting",
  vendor_name: "AWS",
  category: "cloud",
  expense_kind: "recurring",
  recurrence_interval: "monthly",
  recurrence_parent_expense_id: null,
  recurrence_instance_on: null,
  status: "active",
  amount_cents: 12000,
  business_amount_cents: 9000,
  personal_amount_cents: 3000,
  currency: "USD",
  incurred_on: "2026-06-01",
  service_period_start: "2026-06-01",
  service_period_end: "2026-06-30",
  business_use_percent_bps: 7500,
  source_message_id: null,
  source_attachment_id: null,
  source_asset_id: null,
  notes: "",
  created_at: "now",
  updated_at: "now",
};

const receivable: FinanceReceivable = {
  id: "receivable-1",
  contact_id: "contact-1",
  title: "Client session",
  status: "owed",
  amount_cents: 25000,
  currency: "USD",
  issued_on: "2026-06-01",
  due_on: "2026-06-15",
  paid_on: null,
  source_message_id: null,
  source_booking_id: null,
  source_asset_id: null,
  external_reference: "Venmo",
  notes: "",
  created_at: "now",
  updated_at: "now",
};

const summary: FinanceSummary = {
  tax_year: 2026,
  gross_expense_cents: 12000,
  business_expense_cents: 9000,
  personal_expense_cents: 3000,
  receivable_owed_cents: 25000,
  receivable_paid_cents: 0,
  category_totals: [
    {
      category: "cloud",
      gross_cents: 12000,
      business_cents: 9000,
      personal_cents: 3000,
    },
  ],
  vendor_totals: [
    {
      vendor_name: "AWS",
      gross_cents: 12000,
      business_cents: 9000,
      personal_cents: 3000,
    },
  ],
};

afterEach(() => cleanup());

describe("FinanceView", () => {
  it("renders tax summary expenses and receivables", async () => {
    render(<FinanceView apiClient={api()} />);

    expect(await screen.findByText("$90.00")).toBeInTheDocument();
    expect(screen.getByText("Cloud hosting")).toBeInTheDocument();
    expect(screen.getByText("Client session")).toBeInTheDocument();
    expect(screen.getByText("$90.00 business / 75%")).toBeInTheDocument();
  });

  it("creates expenses and updates receivable status without payment processing", async () => {
    const user = userEvent.setup();
    const calls: string[] = [];
    render(<FinanceView apiClient={api(calls)} />);

    await user.type((await screen.findAllByLabelText("Title"))[0], "AI tools");
    await user.type(screen.getByLabelText("Vendor"), "OpenAI");
    await user.clear(screen.getAllByLabelText("Amount")[0]);
    await user.type(screen.getAllByLabelText("Amount")[0], "20.00");
    await user.click(screen.getByRole("button", { name: "Add expense" }));

    const receivableArticle =
      within(await section("Client owed/paid"))
        .getByText("Client session")
        .closest("article") ?? document.body;
    await user.selectOptions(
      within(receivableArticle).getByRole("combobox"),
      "paid",
    );

    expect(calls).toContain("create-expense:AI tools:2000");
    expect(calls).toContain("receivable-status:receivable-1:paid");
  });

  it("records recurring expense occurrences with independent amounts", async () => {
    const user = userEvent.setup();
    const calls: string[] = [];
    render(<FinanceView apiClient={api(calls)} />);

    const expenseArticle =
      within(await section("Expenses"))
        .getByText("Cloud hosting")
        .closest("article") ?? document.body;
    const amount = within(expenseArticle).getByLabelText("Occurrence amount");
    const date = within(expenseArticle).getByLabelText("Occurrence date");
    await user.clear(amount);
    await user.type(amount, "135.42");
    await user.clear(date);
    await user.type(date, "2026-07-01");
    await user.click(
      within(expenseArticle).getByRole("button", {
        name: "Record occurrence",
      }),
    );

    expect(calls).toContain("occurrence:expense-1:13542:2026-07-01");
  });
});

async function section(name: string) {
  const headings = await screen.findAllByRole("heading", { name });
  const heading = headings[headings.length - 1];
  return heading.closest("section") ?? document.body;
}

function api(calls: string[] = []): FinanceApi {
  return {
    listFinanceExpenses: async () => [expense],
    createFinanceExpense: async (request) => {
      calls.push(`create-expense:${request.title}:${request.amount_cents}`);
      return { ...expense, ...request, id: "expense-2" };
    },
    createFinanceExpenseOccurrence: async (id, request) => {
      calls.push(
        `occurrence:${id}:${request.amount_cents}:${request.incurred_on}`,
      );
      return {
        ...expense,
        ...request,
        id: "expense-3",
        recurrence_parent_expense_id: id,
        recurrence_instance_on: request.incurred_on,
      };
    },
    updateFinanceExpense: async (id, request) => ({
      ...expense,
      ...request,
      id,
    }),
    listFinanceReceivables: async () => [receivable],
    createFinanceReceivable: async (request) => ({
      ...receivable,
      ...request,
      id: "receivable-2",
    }),
    updateFinanceReceivable: async (id, request) => {
      calls.push(`receivable-status:${id}:${request.status}`);
      return { ...receivable, ...request, id };
    },
    getFinanceSummary: async () => summary,
    listContacts: async () => [contact],
  };
}
