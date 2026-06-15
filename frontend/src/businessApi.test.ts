import { describe, expect, it } from "vitest";
import { createApiClient } from "./api";

type RecordedRequest = {
  url: string;
  init: RequestInit;
};

function jsonResponse(body: unknown) {
  return new Response(JSON.stringify(body), {
    headers: { "content-type": "application/json" },
  });
}

function clientWithFetch() {
  const requests: RecordedRequest[] = [];
  const client = createApiClient({
    baseUrl: "https://api.mail.ahara.io",
    getAccessToken: () => "token-123",
    fetchImpl: async (input, init = {}) => {
      requests.push({ url: String(input), init });
      return jsonResponse([]);
    },
  });
  return { client, requests };
}

function bodyOf(request: RecordedRequest) {
  return JSON.parse(String(request.init.body));
}

describe("business API routes", () => {
  it("calls calendar and booking routes", async () => {
    const { client, requests } = clientWithFetch();

    await client.listCalendarEvents();
    await client.createCalendarEvent({
      title: "Intro",
      starts_at: "2026-06-13T14:00:00Z",
      ends_at: "2026-06-13T14:30:00Z",
      contact_id: "contact-1",
    });
    await client.updateCalendarEvent("event-1", { status: "confirmed" });
    await client.listCalendarIcsCandidates();
    await client.listBookings();
    await client.createBooking({
      title: "Session",
      starts_at: "2026-06-13T14:00:00Z",
      ends_at: "2026-06-13T14:30:00Z",
    });
    await client.updateBooking("booking-1", { status: "completed" });

    expect(
      requests.map((request) => [request.init.method ?? "GET", request.url]),
    ).toEqual([
      ["GET", "https://api.mail.ahara.io/calendar/events"],
      ["POST", "https://api.mail.ahara.io/calendar/events"],
      ["PATCH", "https://api.mail.ahara.io/calendar/events/event-1"],
      ["GET", "https://api.mail.ahara.io/calendar/ics-candidates"],
      ["GET", "https://api.mail.ahara.io/bookings"],
      ["POST", "https://api.mail.ahara.io/bookings"],
      ["PATCH", "https://api.mail.ahara.io/bookings/booking-1"],
    ]);
    expect(bodyOf(requests[1])).toMatchObject({ title: "Intro" });
    expect(bodyOf(requests[6])).toEqual({ status: "completed" });
  });

  it("calls forwarding audit routes", async () => {
    const { client, requests } = clientWithFetch();

    await client.listForwardingRuleStatuses();
    await client.listForwardingMessageStatuses(25);

    expect(requests.map((request) => request.url)).toEqual([
      "https://api.mail.ahara.io/forwarding/audit/rules",
      "https://api.mail.ahara.io/forwarding/audit/messages?limit=25",
    ]);
  });
});

describe("finance API routes", () => {
  it("calls finance routes", async () => {
    const { client, requests } = clientWithFetch();

    await client.listFinanceExpenses({ tax_year: 2026, limit: 250 });
    await client.createFinanceExpense({
      title: "AI tools",
      category: "ai",
      amount_cents: 2000,
      incurred_on: "2026-06-01",
      business_use_percent_bps: 8000,
    });
    await client.createFinanceExpenseOccurrence("expense-1", {
      amount_cents: 13542,
      incurred_on: "2026-07-01",
    });
    await client.updateFinanceExpense("expense-1", { status: "ended" });
    await client.listFinanceReceivables({ status: "owed" });
    await client.createFinanceReceivable({
      title: "Client session",
      amount_cents: 25000,
    });
    await client.updateFinanceReceivable("receivable-1", {
      paid_on: "2026-06-15",
      status: "paid",
    });
    await client.getFinanceSummary(2026);

    expect(
      requests.map((request) => [request.init.method ?? "GET", request.url]),
    ).toEqual([
      [
        "GET",
        "https://api.mail.ahara.io/finance/expenses?tax_year=2026&limit=250",
      ],
      ["POST", "https://api.mail.ahara.io/finance/expenses"],
      [
        "POST",
        "https://api.mail.ahara.io/finance/expenses/expense-1/occurrences",
      ],
      ["PATCH", "https://api.mail.ahara.io/finance/expenses/expense-1"],
      ["GET", "https://api.mail.ahara.io/finance/receivables?status=owed"],
      ["POST", "https://api.mail.ahara.io/finance/receivables"],
      ["PATCH", "https://api.mail.ahara.io/finance/receivables/receivable-1"],
      ["GET", "https://api.mail.ahara.io/finance/summary?tax_year=2026"],
    ]);
    expect(bodyOf(requests[1])).toMatchObject({ title: "AI tools" });
    expect(bodyOf(requests[2])).toEqual({
      amount_cents: 13542,
      incurred_on: "2026-07-01",
    });
    expect(bodyOf(requests[6])).toEqual({
      paid_on: "2026-06-15",
      status: "paid",
    });
  });
});

describe("calendar query API routes", () => {
  it("calls calendar event range queries", async () => {
    const { client, requests } = clientWithFetch();

    await client.listCalendarEvents({
      starts_from: "2026-06-14T00:00:00Z",
      starts_to: "2026-06-21T00:00:00Z",
      limit: 250,
    });

    expect(requests[0].url).toBe(
      "https://api.mail.ahara.io/calendar/events?starts_from=2026-06-14T00%3A00%3A00Z&starts_to=2026-06-21T00%3A00%3A00Z&limit=250",
    );
  });
});

describe("app authorization API routes", () => {
  it("calls app authorization routes", async () => {
    const { client, requests } = clientWithFetch();
    const temporaryPassword = ["Temporary", "Pass", "123"].join("");

    await client.listAppAuthorizationUsers();
    await client.upsertAppAuthorizationUser("operator", {
      display_name: "Operator",
      password: temporaryPassword,
      apps: { "ahara-business-app": "admin" },
    });
    await client.deleteAppAuthorizationUser("operator");

    expect(
      requests.map((request) => [request.init.method ?? "GET", request.url]),
    ).toEqual([
      ["GET", "https://api.mail.ahara.io/app-authorizations/users"],
      ["PUT", "https://api.mail.ahara.io/app-authorizations/users/operator"],
      ["DELETE", "https://api.mail.ahara.io/app-authorizations/users/operator"],
    ]);
    expect(bodyOf(requests[1])).toMatchObject({
      apps: { "ahara-business-app": "admin" },
    });
  });
});
