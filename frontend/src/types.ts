export type AuthResult =
  | "pass"
  | "fail"
  | "neutral"
  | "softfail"
  | "temperror"
  | "permerror"
  | "none";

export type ScanResult = "pass" | "fail" | "gray" | "processing_failed";

export type SecurityDisposition = "accepted" | "quarantined" | "rejected";

export type MailboxMessageSummary = {
  id: string;
  thread_id: string | null;
  from_address: string;
  from_display_name: string;
  subject: string;
  snippet: string;
  received_at: string | null;
  is_read: boolean;
  has_attachments: boolean;
  attachment_count: number;
  contact_id: string | null;
  auth_verdict: AuthResult | null;
  spam_result: ScanResult | null;
  virus_result: ScanResult | null;
  security_disposition: SecurityDisposition;
};

export type MailboxMessageDetail = {
  id: string;
  thread_id: string | null;
  rfc_message_id: string | null;
  in_reply_to: string | null;
  reference_ids: string[];
  from_address: string;
  from_display_name: string;
  subject: string;
  message_date: string | null;
  received_at: string | null;
  body_text: string;
  recipients: MailboxRecipient[];
  attachments: MailboxAttachment[];
  is_read: boolean;
  contact_id: string | null;
  spf_result: AuthResult | null;
  dkim_result: AuthResult | null;
  dmarc_result: AuthResult | null;
  auth_verdict: AuthResult | null;
  spam_result: ScanResult | null;
  virus_result: ScanResult | null;
  security_disposition: SecurityDisposition;
  security_reason: string | null;
};

export type MailboxThreadDetail = {
  thread_id: string;
  normalized_subject: string;
  message_count: number;
  last_activity_at: string | null;
  messages: MailboxMessageDetail[];
};

export type MailboxRecipient = {
  kind: "to" | "cc" | "bcc";
  address: string;
  address_normalized: string;
  display_name: string;
  position: number;
};

export type MailboxAttachment = {
  id: string;
  position: number;
  filename: string;
  display_filename: string;
  content_type: string;
  size_bytes: number | null;
  content_id: string | null;
};

export type MailboxAttachmentDownload = MailboxAttachment & {
  message_id: string;
  size_bytes: number;
  content_base64: string;
};

export type Contact = {
  id: string;
  display_name: string;
  primary_address: string | null;
  primary_address_normalized: string | null;
  notes: string;
};

export type CreateContactRequest = {
  display_name: string;
} & Partial<{
  primary_address: string | null;
  notes: string | null;
}>;

export type UpdateContactRequest = Partial<{
  display_name: string;
  primary_address: string | null;
  notes: string;
}>;

export type AppAuthorizationUser = {
  username: string;
  email: string | null;
  display_name: string | null;
  apps: Record<string, string>;
};

export type UpsertAppAuthorizationUserRequest = {
  apps: Record<string, string>;
} & Partial<{
  password: string | null;
  email: string | null;
  display_name: string | null;
}>;

export type RoutingPolicy = "allowlist" | "catchall";

export type DomainConfig = {
  domain_name: string;
  routing_policy: RoutingPolicy;
  active: boolean;
  raw_retention_days: number | null;
  addresses: AcceptedAddress[];
};

export type AcceptedAddress = {
  local_part: string;
  active: boolean;
  raw_retention_days: number | null;
};

export type UpdateDomainRequest = Partial<{
  routing_policy: RoutingPolicy;
  active: boolean;
  raw_retention_days: number | null;
}>;

export type UpdateAddressRequest = Partial<{
  active: boolean;
  raw_retention_days: number | null;
}>;

export type OutboundAttachmentInput = {
  filename: string;
  content_type: string;
  content_base64: string;
};

export type OutboundRecipient = {
  kind: "to" | "cc" | "bcc";
  address: string;
  address_normalized: string;
  display_name: string;
  position: number;
};

export type OutboundMessageStatus =
  | "queued"
  | "sending"
  | "sent"
  | "failed"
  | "bounced"
  | "complained";

export type ComposeMessageRequest = {
  from_address: string;
  to: string[];
  subject: string;
  body_text: string;
} & Partial<{
  cc: string[];
  bcc: string[];
  attachments: OutboundAttachmentInput[];
}>;

export type ReplyMessageRequest = {
  from_address: string;
  body_text: string;
} & Partial<{
  to: string[];
  cc: string[];
  bcc: string[];
  attachments: OutboundAttachmentInput[];
}>;

export type OutboundMessageQueued = {
  message_id: string;
  work_id: string;
  rfc_message_id: string;
  status: OutboundMessageStatus;
  recipients: OutboundRecipient[];
};

export type OutboundMessageSummary = {
  id: string;
  thread_id: string | null;
  status: OutboundMessageStatus;
  from_address: string;
  subject: string;
  snippet: string;
  primary_recipient: string | null;
  recipient_count: number;
  last_error: string | null;
  sent_at: string | null;
  created_at: string;
};

export type OutboundMessageDetail = {
  id: string;
  source_message_id: string | null;
  thread_id: string | null;
  rfc_message_id: string;
  in_reply_to: string | null;
  reference_ids: string[];
  status: OutboundMessageStatus;
  from_address: string;
  from_address_normalized: string;
  subject: string;
  body_text: string;
  recipients: OutboundRecipient[];
  attachments: OutboundAttachmentInput[];
  last_error: string | null;
  sent_at: string | null;
  created_at: string;
};

export type ForwardingRuleKind = "domain" | "address";

export type ForwardingRule = {
  id: string;
  rule_kind: ForwardingRuleKind;
  domain_name: string;
  local_part: string | null;
  address_id: string | null;
  target_address: string;
  target_address_normalized: string;
  sender_address_normalized: string | null;
  plus_tag: string | null;
  require_auth_pass: boolean;
  active: boolean;
  created_at: string | null;
  updated_at: string | null;
};

export type UpsertForwardingRuleRequest = {
  domain_name: string;
  target_address: string;
} & Partial<{
  local_part: string | null;
  sender_address: string | null;
  plus_tag: string | null;
  require_auth_pass: boolean;
}>;

export type CalendarEventStatus =
  | "tentative"
  | "confirmed"
  | "canceled"
  | "completed"
  | "missed";
export type BookingStatus =
  | "requested"
  | "confirmed"
  | "canceled"
  | "completed"
  | "missed";

export type CalendarEvent = {
  id: string;
  title: string;
  status: CalendarEventStatus;
  starts_at: string;
  ends_at: string;
  timezone: string;
  location: string;
  description: string;
  contact_id: string | null;
  source_message_id: string | null;
  source_attachment_id: string | null;
  created_at: string;
  updated_at: string;
};

export type Booking = {
  id: string;
  calendar_event_id: string | null;
  contact_id: string | null;
  title: string;
  status: BookingStatus;
  starts_at: string;
  ends_at: string;
  location: string;
  notes: string;
  created_at: string;
  updated_at: string;
};
export type IcsCandidate = {
  message_id: string;
  attachment_id: string;
  filename: string;
  content_type: string;
  size_bytes: number | null;
  subject: string;
  from_address: string;
  received_at: string | null;
  contact_id: string | null;
  suggested_title: string | null;
  suggested_starts_at: string | null;
  suggested_ends_at: string | null;
  suggested_timezone: string | null;
  suggested_location: string | null;
  suggested_description: string | null;
  suggested_status: CalendarEventStatus | null;
  parse_error: string | null;
};

export type CreateCalendarEventRequest = {
  title: string;
  starts_at: string;
  ends_at: string;
} & Partial<{
  timezone: string | null;
  location: string | null;
  description: string | null;
  contact_id: string | null;
  source_message_id: string | null;
  source_attachment_id: string | null;
  status: CalendarEventStatus;
}>;

export type UpdateCalendarEventRequest = Partial<
  Omit<CreateCalendarEventRequest, "title" | "starts_at" | "ends_at"> & {
    title: string;
    starts_at: string;
    ends_at: string;
  }
>;

export type CreateBookingRequest = {
  title: string;
  starts_at: string;
  ends_at: string;
} & Partial<{
  calendar_event_id: string | null;
  contact_id: string | null;
  location: string | null;
  notes: string | null;
  status: BookingStatus;
}>;

export type UpdateBookingRequest = Partial<
  Omit<CreateBookingRequest, "title" | "starts_at" | "ends_at"> & {
    title: string;
    starts_at: string;
    ends_at: string;
  }
>;

export type ForwardingRuleStatus = {
  rule_id: string;
  rule_kind: string;
  domain_name: string;
  local_part: string | null;
  target_address: string;
  active: boolean;
  queued_count: number;
  sending_count: number;
  sent_count: number;
  failed_count: number;
  bounced_count: number;
  complained_count: number;
  last_attempt_at: string | null;
  last_error: string | null;
};

export type ForwardingMessageStatus = {
  source_message_id: string;
  thread_id: string | null;
  subject: string;
  from_address: string;
  received_at: string | null;
  matching_rule_count: number;
  queued_count: number;
  sending_count: number;
  sent_count: number;
  failed_count: number;
  bounced_count: number;
  complained_count: number;
  last_error: string | null;
};
