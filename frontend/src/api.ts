import { config } from "./config";
import type {
  AcceptedAddress,
  ComposeMessageRequest,
  Contact,
  CreateContactRequest,
  DomainConfig,
  ForwardingRule,
  MailboxMessageDetail,
  MailboxMessageSummary,
  MailboxThreadDetail,
  OutboundMessageDetail,
  OutboundMessageQueued,
  OutboundMessageSummary,
  ReplyMessageRequest,
  UpdateContactRequest,
  UpdateDomainRequest,
  UpsertForwardingRuleRequest,
} from "./types";

export type AccessTokenRequest = {
  forceRefresh?: boolean;
};

type FetchLike = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>;

export type ApiClientOptions = {
  baseUrl?: string;
  getAccessToken: (
    request?: AccessTokenRequest,
  ) => Promise<string | undefined> | string | undefined;
  fetchImpl?: FetchLike;
};

export class ApiClientError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.status = status;
    this.code = code;
    this.name = "ApiClientError";
  }
}

export class ApiClient {
  private readonly baseUrl: string;
  private readonly fetchImpl: FetchLike;
  private readonly options: ApiClientOptions;

  constructor(options: ApiClientOptions) {
    this.options = options;
    this.baseUrl = (options.baseUrl ?? config.apiBaseUrl).replace(/\/$/, "");
    this.fetchImpl = options.fetchImpl ?? defaultFetch;
  }

  fetchMailboxMessages(query: { limit?: number; unread_only?: boolean } = {}) {
    const params = new URLSearchParams();
    if (query.limit !== undefined) {
      params.set("limit", String(query.limit));
    }
    if (query.unread_only !== undefined) {
      params.set("unread_only", String(query.unread_only));
    }
    return this.request<MailboxMessageSummary[]>(
      `/mailbox/messages${queryString(params)}`,
    );
  }

  fetchMessageDetail(messageId: string) {
    return this.request<MailboxMessageDetail>(
      `/mailbox/messages/${encodeURIComponent(messageId)}`,
    );
  }

  fetchThreadDetail(threadId: string) {
    return this.request<MailboxThreadDetail>(
      `/mailbox/threads/${encodeURIComponent(threadId)}`,
    );
  }

  searchMessages(query: string, limit?: number) {
    const params = new URLSearchParams({ q: query });
    if (limit !== undefined) {
      params.set("limit", String(limit));
    }
    return this.request<MailboxMessageSummary[]>(
      `/mailbox/search${queryString(params)}`,
    );
  }

  updateMessageState(messageId: string, isRead: boolean) {
    return this.request<MailboxMessageSummary>(
      `/mailbox/messages/${encodeURIComponent(messageId)}/state`,
      {
        method: "PATCH",
        body: { is_read: isRead },
      },
    );
  }

  linkMessageContact(messageId: string, contactId: string | null) {
    return this.request<MailboxMessageSummary>(
      `/mailbox/messages/${encodeURIComponent(messageId)}/contact`,
      {
        method: "PATCH",
        body: { contact_id: contactId },
      },
    );
  }

  listContacts() {
    return this.request<Contact[]>("/contacts");
  }

  createContact(request: CreateContactRequest) {
    return this.request<Contact>("/contacts", {
      method: "POST",
      body: request,
    });
  }

  updateContact(contactId: string, request: UpdateContactRequest) {
    return this.request<Contact>(`/contacts/${encodeURIComponent(contactId)}`, {
      method: "PATCH",
      body: request,
    });
  }

  listDomains() {
    return this.request<DomainConfig[]>("/domains");
  }

  updateDomain(domainName: string, request: UpdateDomainRequest) {
    return this.request<DomainConfig>(
      `/domains/${encodeURIComponent(domainName)}`,
      {
        method: "PATCH",
        body: request,
      },
    );
  }

  addAddress(domainName: string, localPart: string) {
    return this.request<AcceptedAddress>(
      `/domains/${encodeURIComponent(domainName)}/addresses`,
      {
        method: "POST",
        body: { local_part: localPart },
      },
    );
  }

  deactivateAddress(domainName: string, localPart: string) {
    return this.request<AcceptedAddress>(
      `/domains/${encodeURIComponent(domainName)}/addresses/${encodeURIComponent(localPart)}`,
      { method: "DELETE" },
    );
  }

  composeMessage(request: ComposeMessageRequest) {
    return this.request<OutboundMessageQueued>("/outbound/messages/compose", {
      method: "POST",
      body: request,
    });
  }

  replyToMessage(messageId: string, request: ReplyMessageRequest) {
    return this.request<OutboundMessageQueued>(
      `/mailbox/messages/${encodeURIComponent(messageId)}/reply`,
      {
        method: "POST",
        body: request,
      },
    );
  }

  listOutboundMessages() {
    return this.request<OutboundMessageSummary[]>("/outbound/messages");
  }

  fetchOutboundMessage(messageId: string) {
    return this.request<OutboundMessageDetail>(
      `/outbound/messages/${encodeURIComponent(messageId)}`,
    );
  }

  listForwardingRules() {
    return this.request<ForwardingRule[]>("/forwarding/rules");
  }

  upsertForwardingRule(request: UpsertForwardingRuleRequest) {
    return this.request<ForwardingRule>("/forwarding/rules", {
      method: "POST",
      body: request,
    });
  }

  deactivateForwardingRule(ruleId: string) {
    return this.request<ForwardingRule>(
      `/forwarding/rules/${encodeURIComponent(ruleId)}`,
      { method: "DELETE" },
    );
  }

  private async request<T>(
    path: string,
    options: { method?: string; body?: unknown } = {},
  ): Promise<T> {
    const token = await this.options.getAccessToken();
    if (!token) {
      throw new ApiClientError(401, "unauthorized", "missing access token");
    }

    const { method, headers, body } = requestParts(token, options);
    let response = await this.fetchImpl(`${this.baseUrl}${path}`, {
      method,
      headers,
      body,
    });
    if (response.status === 401) {
      const refreshedToken = await this.options.getAccessToken({
        forceRefresh: true,
      });
      if (refreshedToken) {
        const retry = requestParts(refreshedToken, options);
        response = await this.fetchImpl(`${this.baseUrl}${path}`, retry);
      }
    }
    if (!response.ok) {
      throw await apiError(response);
    }
    if (response.status === 204) {
      return undefined as T;
    }
    return (await response.json()) as T;
  }
}

function requestParts(
  token: string,
  options: { method?: string; body?: unknown },
) {
  const headers = new Headers({
    authorization: `Bearer ${token}`,
  });
  let body: string | undefined;
  if (options.body !== undefined) {
    headers.set("content-type", "application/json");
    body = JSON.stringify(options.body);
  }

  return {
    method: options.method ?? "GET",
    headers,
    body,
  };
}

export function createApiClient(options: ApiClientOptions) {
  return new ApiClient(options);
}

async function apiError(response: Response) {
  let payload: { code?: string; message?: string } = {};
  try {
    payload = (await response.json()) as { code?: string; message?: string };
  } catch {
    payload = {};
  }
  return new ApiClientError(
    response.status,
    payload.code ?? "api_error",
    payload.message ?? response.statusText,
  );
}

function queryString(params: URLSearchParams) {
  const value = params.toString();
  return value ? `?${value}` : "";
}

function defaultFetch(input: RequestInfo | URL, init?: RequestInit) {
  return globalThis.fetch(input, init);
}
