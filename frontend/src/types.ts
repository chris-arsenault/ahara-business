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

export type RoutingPolicy = "allowlist" | "catchall";

export type DomainConfig = {
  domain_name: string;
  routing_policy: RoutingPolicy;
  active: boolean;
  addresses: AcceptedAddress[];
};

export type AcceptedAddress = {
  local_part: string;
  active: boolean;
};

export type UpdateDomainRequest = Partial<{
  routing_policy: RoutingPolicy;
  active: boolean;
}>;

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
}>;

export type ReplyMessageRequest = {
  from_address: string;
  body_text: string;
} & Partial<{
  to: string[];
  cc: string[];
  bcc: string[];
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
  last_error: string | null;
  sent_at: string | null;
  created_at: string;
};

export type ForwardingRule = {
  id: string;
  domain_name: string;
  local_part: string;
  address_id: string;
  target_address: string;
  target_address_normalized: string;
  active: boolean;
  created_at: string | null;
  updated_at: string | null;
};

export type UpsertForwardingRuleRequest = {
  domain_name: string;
  local_part: string;
  target_address: string;
};
