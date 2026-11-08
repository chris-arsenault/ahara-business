import type {
  MailboxMessageDetail,
  MailboxMessageSummary,
  MailboxThreadDetail,
  OutboundMessageSummary,
  OutboundMessageDetail,
} from "./types";

export function updateThreadSummary(
  thread: MailboxThreadDetail,
  updated: MailboxMessageSummary,
): MailboxThreadDetail {
  return {
    ...thread,
    messages: thread.messages.map((message) =>
      message.id === updated.id
        ? {
            ...message,
            is_read: updated.is_read,
            contact_id: updated.contact_id,
          }
        : message,
    ),
  };
}

export function conversationRows(messages: MailboxMessageSummary[]) {
  const groups = new Map<string, MailboxMessageSummary[]>();
  for (const message of messages) {
    const key = message.thread_id ?? message.id;
    groups.set(key, [...(groups.get(key) ?? []), message]);
  }
  return [...groups.values()].map((group) => ({
    message: {
      ...group[0],
      is_read: group.every((message) => message.is_read),
      attachment_count: group.reduce(
        (total, message) => total + message.attachment_count,
        0,
      ),
    },
    count: group.length,
  }));
}

export function messageTime(message: MailboxMessageDetail) {
  return message.received_at ?? message.sent_at ?? message.created_at ?? null;
}

export function sentConversationRows(messages: OutboundMessageSummary[]) {
  const groups = new Map<
    string,
    { message: OutboundMessageSummary; count: number }
  >();
  for (const message of messages) {
    const key = message.thread_id ?? message.id;
    const existing = groups.get(key);
    if (existing) existing.count += 1;
    else groups.set(key, { message, count: 1 });
  }
  return [...groups.values()];
}

export function standaloneOutboundThread(
  message: OutboundMessageDetail,
): MailboxThreadDetail {
  return {
    thread_id: message.thread_id ?? message.id,
    normalized_subject: message.subject,
    message_count: 1,
    last_activity_at: message.sent_at ?? message.created_at,
    messages: [
      {
        id: message.id,
        direction: "outbound",
        status: message.status,
        sent_at: message.sent_at,
        created_at: message.created_at,
        received_at: null,
        thread_id: message.thread_id,
        rfc_message_id: message.rfc_message_id,
        in_reply_to: message.in_reply_to,
        reference_ids: message.reference_ids,
        from_address: message.from_address,
        from_display_name: "",
        subject: message.subject,
        message_date: message.sent_at ?? message.created_at,
        body_text: message.body_text,
        is_read: true,
        contact_id: null,
        recipients: message.recipients,
        attachments: message.attachments.map((attachment) => ({
          ...attachment,
          id: `outbound:${attachment.position}`,
          content_id: null,
        })),
        spf_result: null,
        dkim_result: null,
        dmarc_result: null,
        auth_verdict: null,
        spam_result: null,
        virus_result: null,
        security_disposition: "accepted",
        security_reason: "outbound",
      },
    ],
  };
}

export function mailDate(value: string | null, compact = false) {
  if (!value) return "Pending";
  const date = new Date(
    value.replace(" ", "T").replace(/([+-]\d{2})$/, "$1:00"),
  );
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(
    undefined,
    compact
      ? { month: "short", day: "numeric" }
      : { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" },
  ).format(date);
}
