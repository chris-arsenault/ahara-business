/* eslint-disable max-lines-per-function */
import { afterEach, describe, expect, it } from "vitest";
import { ApiClient, ApiClientError } from "./api";

type RecordedRequest = {
  url: string;
  init: RequestInit;
};

function jsonResponse(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function clientWithFetch(
  handler: (request: RecordedRequest) => Response | Promise<Response>,
) {
  const requests: RecordedRequest[] = [];
  const client = new ApiClient({
    baseUrl: "https://api.mail.ahara.io/",
    getAccessToken: () => "token-123",
    fetchImpl: async (input, init = {}) => {
      const request = { url: String(input), init };
      requests.push(request);
      return handler(request);
    },
  });
  return { client, requests };
}

function bodyOf(request: RecordedRequest) {
  return JSON.parse(String(request.init.body));
}

const originalFetch = globalThis.fetch;

afterEach(() => {
  globalThis.fetch = originalFetch;
});

describe("ApiClient", () => {
  it("injects bearer tokens on app API calls", async () => {
    const { client, requests } = clientWithFetch(() => jsonResponse([]));

    await client.listDomains();

    expect(requests[0].url).toBe("https://api.mail.ahara.io/domains");
    expect(new Headers(requests[0].init.headers).get("authorization")).toBe(
      "Bearer token-123",
    );
  });

  it("force refreshes the token and retries once after a 401", async () => {
    const tokenRequests: unknown[] = [];
    const requests: RecordedRequest[] = [];
    const client = new ApiClient({
      baseUrl: "https://api.mail.ahara.io",
      getAccessToken: (request) => {
        tokenRequests.push(request);
        return request?.forceRefresh ? "fresh-token" : "expired-token";
      },
      fetchImpl: async (input, init = {}) => {
        const request = { url: String(input), init };
        requests.push(request);
        return requests.length === 1
          ? jsonResponse({ code: "unauthorized", message: "expired" }, 401)
          : jsonResponse([]);
      },
    });

    await client.listDomains();

    expect(tokenRequests).toEqual([undefined, { forceRefresh: true }]);
    expect(requests).toHaveLength(2);
    expect(new Headers(requests[0].init.headers).get("authorization")).toBe(
      "Bearer expired-token",
    );
    expect(new Headers(requests[1].init.headers).get("authorization")).toBe(
      "Bearer fresh-token",
    );
  });

  it("calls default fetch with the global receiver", async () => {
    let calledWithGlobalReceiver = false;
    globalThis.fetch = function fetchWithReceiverCheck(
      this: typeof globalThis,
    ) {
      calledWithGlobalReceiver = this === globalThis;
      return Promise.resolve(jsonResponse([]));
    } as typeof fetch;
    const client = new ApiClient({
      baseUrl: "https://api.mail.ahara.io",
      getAccessToken: () => "token-123",
    });

    await client.listDomains();

    expect(calledWithGlobalReceiver).toBe(true);
  });

  it("calls mailbox list detail thread and search routes", async () => {
    const { client, requests } = clientWithFetch(() => jsonResponse([]));

    await client.fetchMailboxMessages({ limit: 25, unread_only: true });
    await client.fetchMessageDetail("message-1");
    await client.fetchThreadDetail("thread-1");
    await client.searchMessages("invoice due", 10);

    expect(requests.map((request) => request.url)).toEqual([
      "https://api.mail.ahara.io/mailbox/messages?limit=25&unread_only=true",
      "https://api.mail.ahara.io/mailbox/messages/message-1",
      "https://api.mail.ahara.io/mailbox/threads/thread-1",
      "https://api.mail.ahara.io/mailbox/search?q=invoice+due&limit=10",
    ]);
  });

  it("calls read-state and contact mutation routes", async () => {
    const { client, requests } = clientWithFetch(() => jsonResponse({}));

    await client.updateMessageState("message-1", true);
    await client.linkMessageContact("message-1", "contact-1");
    await client.linkMessageContact("message-1", null);

    expect(requests[0].init.method).toBe("PATCH");
    expect(requests[0].url).toBe(
      "https://api.mail.ahara.io/mailbox/messages/message-1/state",
    );
    expect(bodyOf(requests[0])).toEqual({ is_read: true });
    expect(requests[1].url).toBe(
      "https://api.mail.ahara.io/mailbox/messages/message-1/contact",
    );
    expect(bodyOf(requests[1])).toEqual({ contact_id: "contact-1" });
    expect(bodyOf(requests[2])).toEqual({ contact_id: null });
  });

  it("calls contact and domain admin routes", async () => {
    const { client, requests } = clientWithFetch(() => jsonResponse({}));

    await client.listContacts();
    await client.createContact({ display_name: "Chris" });
    await client.updateContact("contact-1", { notes: "updated" });
    await client.listDomains();
    await client.updateDomain("ahara.io", { routing_policy: "catchall" });
    await client.addAddress("ahara.io", "support");
    await client.deactivateAddress("ahara.io", "support");

    expect(
      requests.map((request) => [request.init.method ?? "GET", request.url]),
    ).toEqual([
      ["GET", "https://api.mail.ahara.io/contacts"],
      ["POST", "https://api.mail.ahara.io/contacts"],
      ["PATCH", "https://api.mail.ahara.io/contacts/contact-1"],
      ["GET", "https://api.mail.ahara.io/domains"],
      ["PATCH", "https://api.mail.ahara.io/domains/ahara.io"],
      ["POST", "https://api.mail.ahara.io/domains/ahara.io/addresses"],
      [
        "DELETE",
        "https://api.mail.ahara.io/domains/ahara.io/addresses/support",
      ],
    ]);
    expect(bodyOf(requests[5])).toEqual({ local_part: "support" });
  });

  it("calls outbound compose reply and status routes", async () => {
    const { client, requests } = clientWithFetch(() => jsonResponse({}));

    await client.composeMessage({
      from_address: "contact@ahara.io",
      to: ["person@example.com"],
      cc: [],
      bcc: [],
      subject: "Plain note",
      body_text: "body",
    });
    await client.replyToMessage("message-1", {
      from_address: "contact@ahara.io",
      body_text: "reply",
    });
    await client.listOutboundMessages();
    await client.fetchOutboundMessage("outbound-1");

    expect(
      requests.map((request) => [request.init.method ?? "GET", request.url]),
    ).toEqual([
      ["POST", "https://api.mail.ahara.io/outbound/messages/compose"],
      ["POST", "https://api.mail.ahara.io/mailbox/messages/message-1/reply"],
      ["GET", "https://api.mail.ahara.io/outbound/messages"],
      ["GET", "https://api.mail.ahara.io/outbound/messages/outbound-1"],
    ]);
    expect(bodyOf(requests[0])).toEqual({
      from_address: "contact@ahara.io",
      to: ["person@example.com"],
      cc: [],
      bcc: [],
      subject: "Plain note",
      body_text: "body",
    });
    expect(bodyOf(requests[1])).toEqual({
      from_address: "contact@ahara.io",
      body_text: "reply",
    });
  });

  it("calls forwarding rule routes", async () => {
    const { client, requests } = clientWithFetch(() => jsonResponse({}));

    await client.listForwardingRules();
    await client.upsertForwardingRule({
      domain_name: "ahara.io",
      local_part: "contact",
      target_address: "target@example.com",
    });
    await client.deactivateForwardingRule("rule-1");

    expect(
      requests.map((request) => [request.init.method ?? "GET", request.url]),
    ).toEqual([
      ["GET", "https://api.mail.ahara.io/forwarding/rules"],
      ["POST", "https://api.mail.ahara.io/forwarding/rules"],
      ["DELETE", "https://api.mail.ahara.io/forwarding/rules/rule-1"],
    ]);
    expect(bodyOf(requests[1])).toEqual({
      domain_name: "ahara.io",
      local_part: "contact",
      target_address: "target@example.com",
    });
  });

  it("normalizes API errors", async () => {
    const { client } = clientWithFetch(() =>
      jsonResponse({ code: "validation_error", message: "bad request" }, 400),
    );

    await expect(client.listDomains()).rejects.toMatchObject<ApiClientError>({
      status: 400,
      code: "validation_error",
      message: "bad request",
    });
  });

  it("rejects calls without an access token before fetch", async () => {
    const client = new ApiClient({
      baseUrl: "https://api.mail.ahara.io",
      getAccessToken: () => undefined,
      fetchImpl: async () => jsonResponse([]),
    });

    await expect(client.listDomains()).rejects.toMatchObject<ApiClientError>({
      status: 401,
      code: "unauthorized",
    });
  });
});
