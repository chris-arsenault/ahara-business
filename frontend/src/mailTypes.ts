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
  direction: "inbound" | "outbound";
  status: OutboundMessageStatus | "received";
  sent_at: string | null;
  created_at: string | null;
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

export type OutboundMessageStatus =
  | "queued"
  | "sending"
  | "sent"
  | "failed"
  | "bounced"
  | "complained";
