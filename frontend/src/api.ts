import { config } from "./config";
import { attachAccessApi, type AccessApiSurface } from "./accessApi";
import { attachFinanceApi, type FinanceApiSurface } from "./financeApi";
import {
  authenticatedRequest,
  ApiClientError,
  type ApiClientOptions,
  type ApiRequestOptions,
} from "./apiCore";
import type {
  AcceptedAddress,
  AppAuthorizationUser,
  Booking,
  ComposeMessageRequest,
  Contact,
  CalendarEvent,
  CalendarEventStatus,
  CreateBookingRequest,
  CreateCalendarEventRequest,
  CreateContactRequest,
  DomainConfig,
  ForwardingMessageStatus,
  ForwardingRule,
  ForwardingRuleStatus,
  IcsCandidate,
  MailboxAttachmentDownload,
  MailboxMessageDetail,
  MailboxMessageSummary,
  MailboxThreadDetail,
  OutboundMessageDetail,
  OutboundMessageQueued,
  OutboundMessageSummary,
  ReplyMessageRequest,
  UpdateBookingRequest,
  UpdateCalendarEventRequest,
  UpdateContactRequest,
  UpdateAddressRequest,
  UpsertAppAuthorizationUserRequest,
  UpdateDomainRequest,
  UpsertForwardingRuleRequest,
} from "./types";

type MailboxListQuery = Partial<{
  limit: number;
  unread_only: boolean;
}>;

type CalendarEventListQuery = Partial<{
  contact_id: string;
  status: CalendarEventStatus;
  starts_from: string;
  starts_to: string;
  limit: number;
}>;

export { ApiClientError };
export type { AccessTokenRequest, ApiClientOptions } from "./apiCore";

export class ApiClient {
  private readonly baseUrl: string;
  private readonly options: ApiClientOptions;

  constructor(options: ApiClientOptions) {
    this.options = options;
    this.baseUrl = (options.baseUrl ?? config.apiBaseUrl).replace(/\/$/, "");
  }

  fetchMailboxMessages(query: MailboxListQuery = {}) {
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

  downloadAttachment(messageId: string, attachmentId: string) {
    return this.request<MailboxAttachmentDownload>(
      `/mailbox/messages/${encodeURIComponent(messageId)}/attachments/${encodeURIComponent(attachmentId)}`,
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

  listAppAuthorizationUsers() {
    return this.request<AppAuthorizationUser[]>("/app-authorizations/users");
  }

  upsertAppAuthorizationUser(
    username: string,
    request: UpsertAppAuthorizationUserRequest,
  ) {
    return this.request<AppAuthorizationUser>(
      `/app-authorizations/users/${encodeURIComponent(username)}`,
      {
        method: "PUT",
        body: request,
      },
    );
  }

  deleteAppAuthorizationUser(username: string) {
    return this.request<void>(
      `/app-authorizations/users/${encodeURIComponent(username)}`,
      { method: "DELETE" },
    );
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

  addAddress(
    domainName: string,
    localPart: string,
    rawRetentionDays?: number | null,
  ) {
    return this.request<AcceptedAddress>(
      `/domains/${encodeURIComponent(domainName)}/addresses`,
      {
        method: "POST",
        body: { local_part: localPart, raw_retention_days: rawRetentionDays },
      },
    );
  }

  updateAddress(
    domainName: string,
    localPart: string,
    request: UpdateAddressRequest,
  ) {
    return this.request<AcceptedAddress>(
      `/domains/${encodeURIComponent(domainName)}/addresses/${encodeURIComponent(localPart)}`,
      {
        method: "PATCH",
        body: request,
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

  listForwardingRuleStatuses() {
    return this.request<ForwardingRuleStatus[]>("/forwarding/audit/rules");
  }

  listForwardingMessageStatuses(limit?: number) {
    const params = new URLSearchParams();
    if (limit !== undefined) {
      params.set("limit", String(limit));
    }
    return this.request<ForwardingMessageStatus[]>(
      `/forwarding/audit/messages${queryString(params)}`,
    );
  }

  listCalendarEvents(query: CalendarEventListQuery = {}) {
    const params = new URLSearchParams();
    if (query.contact_id) {
      params.set("contact_id", query.contact_id);
    }
    if (query.status) {
      params.set("status", query.status);
    }
    if (query.starts_from) {
      params.set("starts_from", query.starts_from);
    }
    if (query.starts_to) {
      params.set("starts_to", query.starts_to);
    }
    if (query.limit !== undefined) {
      params.set("limit", String(query.limit));
    }
    return this.request<CalendarEvent[]>(
      `/calendar/events${queryString(params)}`,
    );
  }

  createCalendarEvent(request: CreateCalendarEventRequest) {
    return this.request<CalendarEvent>("/calendar/events", {
      method: "POST",
      body: request,
    });
  }

  updateCalendarEvent(eventId: string, request: UpdateCalendarEventRequest) {
    return this.request<CalendarEvent>(
      `/calendar/events/${encodeURIComponent(eventId)}`,
      { method: "PATCH", body: request },
    );
  }

  listCalendarIcsCandidates() {
    return this.request<IcsCandidate[]>("/calendar/ics-candidates");
  }

  listBookings() {
    return this.request<Booking[]>("/bookings");
  }

  createBooking(request: CreateBookingRequest) {
    return this.request<Booking>("/bookings", {
      method: "POST",
      body: request,
    });
  }

  updateBooking(bookingId: string, request: UpdateBookingRequest) {
    return this.request<Booking>(`/bookings/${encodeURIComponent(bookingId)}`, {
      method: "PATCH",
      body: request,
    });
  }

  private async request<T>(
    path: string,
    options: ApiRequestOptions = {},
  ): Promise<T> {
    return authenticatedRequest<T>({
      baseUrl: this.baseUrl,
      clientOptions: this.options,
      path,
      requestOptions: options,
    });
  }
}

export function createApiClient(options: ApiClientOptions) {
  return attachFinanceApi(
    attachAccessApi(new ApiClient(options), options),
    options,
  );
}

function queryString(params: URLSearchParams) {
  const value = params.toString();
  return value ? `?${value}` : "";
}

export type AppApiClient = ApiClient & AccessApiSurface & FinanceApiSurface;
