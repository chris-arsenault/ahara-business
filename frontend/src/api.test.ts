/* eslint-disable max-lines-per-function */
import { afterEach, describe, expect, it } from "vitest";
import { ApiClient, ApiClientError, createApiClient } from "./api";

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
    await client.createDomain({ domain_name: "ahara.io" });
    await client.updateDomain("ahara.io", { routing_policy: "catchall" });
    await client.addAddress("ahara.io", "support");
    await client.updateAddress("ahara.io", "support", {
      raw_retention_days: null,
    });
    await client.deactivateAddress("ahara.io", "support");

    expect(
      requests.map((request) => [request.init.method ?? "GET", request.url]),
    ).toEqual([
      ["GET", "https://api.mail.ahara.io/contacts"],
      ["POST", "https://api.mail.ahara.io/contacts"],
      ["PATCH", "https://api.mail.ahara.io/contacts/contact-1"],
      ["GET", "https://api.mail.ahara.io/domains"],
      ["POST", "https://api.mail.ahara.io/domains"],
      ["PATCH", "https://api.mail.ahara.io/domains/ahara.io"],
      ["POST", "https://api.mail.ahara.io/domains/ahara.io/addresses"],
      ["PATCH", "https://api.mail.ahara.io/domains/ahara.io/addresses/support"],
      [
        "DELETE",
        "https://api.mail.ahara.io/domains/ahara.io/addresses/support",
      ],
    ]);
    expect(bodyOf(requests[4])).toEqual({ domain_name: "ahara.io" });
    expect(bodyOf(requests[6])).toEqual({
      local_part: "support",
    });
    expect(bodyOf(requests[7])).toEqual({ raw_retention_days: null });
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
      sender_address: "sender@example.com",
      plus_tag: "sales",
      require_auth_pass: false,
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
      sender_address: "sender@example.com",
      plus_tag: "sales",
      require_auth_pass: false,
    });
  });

  it("calls mailbox attachment download route", async () => {
    const { client, requests } = clientWithFetch(() => jsonResponse({}));

    await client.downloadAttachment("message-1", "attachment-1");

    expect(requests.map((request) => request.url)).toEqual([
      "https://api.mail.ahara.io/mailbox/messages/message-1/attachments/attachment-1",
    ]);
  });

  it("calls shared access routes and uploads to a signed URL", async () => {
    const requests: RecordedRequest[] = [];
    const client = createApiClient({
      baseUrl: "https://api.mail.ahara.io",
      accessBaseUrl: "https://api.access.ahara.io",
      getAccessToken: () => "token-123",
      fetchImpl: async (input, init = {}) => {
        const request = { url: String(input), init };
        requests.push(request);
        if (request.url === "https://s3.example.test/upload") {
          return new Response(null, { status: 200 });
        }
        return jsonResponse(accessResponse(request.url));
      },
    });
    const file = new File(["master"], "master.wav", { type: "audio/wav" });

    await client.listAccessPrincipals();
    await client.createAccessPrincipal({
      principal_kind: "external",
      display_name: "Engineer",
      username: "engineer",
    });
    await client.listAccessAudiences();
    await client.createAccessAudience({
      audience_key: "mastering",
      display_name: "Mastering",
    });
    await client.addAccessAudienceMember("audience-1", "principal-1");
    await client.uploadAccessAsset(file, {
      owner_app: "tsonu_music",
      filename: file.name,
      content_type: file.type,
      size_bytes: file.size,
    });
    await client.createAccessGrant({
      asset_id: "asset-1",
      principal_id: "principal-1",
      permission_level: "download",
    });
    await client.revokeAccessGrant("grant-1");

    expect(requests.map((request) => request.url)).toEqual([
      "https://api.access.ahara.io/principals",
      "https://api.access.ahara.io/principals",
      "https://api.access.ahara.io/audiences",
      "https://api.access.ahara.io/audiences",
      "https://api.access.ahara.io/audiences/audience-1/members",
      "https://api.access.ahara.io/assets/upload-url",
      "https://s3.example.test/upload",
      "https://api.access.ahara.io/grants",
      "https://api.access.ahara.io/grants/grant-1/revoke",
    ]);
    expect(bodyOf(requests[4])).toEqual({ principal_id: "principal-1" });
    expect(new Headers(requests[6].init.headers).get("Content-Type")).toBe(
      "audio/wav",
    );
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

function accessResponse(url: string) {
  if (url.endsWith("/assets/upload-url")) {
    return {
      asset: accessAsset,
      upload: {
        url: "https://s3.example.test/upload",
        method: "PUT",
        headers: { "Content-Type": "audio/wav" },
        expires_in_seconds: 900,
      },
    };
  }
  if (url.endsWith("/grants/grant-1/revoke")) {
    return { ...accessGrant, revoked_at: "2026-01-01T00:00:00Z" };
  }
  if (url.endsWith("/grants")) {
    return accessGrant;
  }
  return {};
}

const accessAsset = {
  id: "asset-1",
  owner_app: "tsonu_music",
  resource_id: null,
  storage_kind: "managed_s3",
  filename: "master.wav",
  content_type: "audio/wav",
  size_bytes: 6,
  sha256: null,
  active: true,
};

const accessGrant = {
  id: "grant-1",
  principal_id: "principal-1",
  audience_id: null,
  resource_id: null,
  asset_id: "asset-1",
  permission_level: "download",
  expires_at: null,
  revoked_at: null,
};
