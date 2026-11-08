import { useCallback, useEffect, useRef, useState } from "react";
import { ChevronDown, Mail, Paperclip, Reply, ShieldCheck } from "lucide-react";
import type { MessageDetailProps, ThreadDetailProps } from "./mailboxTypes";
import type { MailboxMessageDetail } from "./types";
import { formatBytes, sanitizeAttachmentName } from "./textRendering";
import { mailDate, messageTime } from "./mailboxPresentation";
import { ReplyForm } from "./replyComposer";

export function ThreadDetail({ thread, ...actions }: ThreadDetailProps) {
  const threadRef = useRef<HTMLElement>(null);
  const latestId = thread.messages.at(-1)?.id;
  useEffect(() => {
    threadRef.current
      ?.querySelector(".conversation-message:last-child")
      ?.scrollIntoView?.({ block: "nearest" });
  }, [latestId]);
  const participants = [
    ...new Set(
      thread.messages.map(
        (message) => message.from_display_name || message.from_address,
      ),
    ),
  ];
  return (
    <section
      className="thread-detail"
      aria-label="Thread detail"
      ref={threadRef}
    >
      <header className="conversation-heading">
        <h2>{thread.messages[0]?.subject || "(no subject)"}</h2>
        <p>
          {participants.join(", ")}{" "}
          <span>
            · {thread.message_count}{" "}
            {thread.message_count === 1 ? "message" : "messages"}
          </span>
        </p>
      </header>
      <div className="conversation-messages">
        {thread.messages.map((message, index) => (
          <MessageDetail
            key={message.id}
            message={message}
            initiallyExpanded={index === thread.messages.length - 1}
            {...actions}
          />
        ))}
      </div>
    </section>
  );
}

export function MessageDetail({
  message,
  initiallyExpanded = true,
  ...actions
}: MessageDetailProps & Partial<{ initiallyExpanded: boolean }>) {
  const [expanded, setExpanded] = useState(initiallyExpanded);
  const [replyOpen, setReplyOpen] = useState(false);
  const toggleExpanded = useCallback(
    () => setExpanded((current) => !current),
    [],
  );
  const toggleReply = useCallback(() => {
    setExpanded(true);
    setReplyOpen((current) => !current);
  }, []);
  const closeReply = useCallback(() => setReplyOpen(false), []);
  const outbound = message.direction === "outbound";
  return (
    <article
      className="conversation-message"
      data-direction={message.direction ?? "inbound"}
      aria-label="Message detail"
    >
      <MessageHeader
        message={message}
        expanded={expanded}
        onToggle={toggleExpanded}
        onReply={!outbound && actions.onReply ? toggleReply : undefined}
      />
      {expanded ? (
        <div className="conversation-message-content">
          <MessageMetadata message={message} />
          <pre className="message-body-text">{message.body_text}</pre>
          <MessageFiles
            message={message}
            onDownloadAttachment={actions.onDownloadAttachment}
          />
          {!outbound ? <MessageActions message={message} {...actions} /> : null}
          {replyOpen && actions.onReply ? (
            <ReplyForm
              message={message}
              onReply={actions.onReply}
              onQueued={closeReply}
              onCancel={closeReply}
              uploadApi={actions.uploadApi ?? {}}
            />
          ) : null}
        </div>
      ) : (
        <p className="collapsed-message-preview">
          {message.body_text.slice(0, 160)}
        </p>
      )}
    </article>
  );
}

function MessageHeader({
  message,
  expanded,
  onToggle,
  onReply,
}: {
  message: MailboxMessageDetail;
  expanded: boolean;
  onToggle: () => void;
} & Partial<{ onReply: () => void }>) {
  const sender = message.from_display_name || message.from_address;
  return (
    <header className="conversation-message-header">
      <button
        className="conversation-toggle"
        type="button"
        onClick={onToggle}
        aria-expanded={expanded}
        aria-label={`${expanded ? "Collapse" : "Expand"} message from ${message.from_address}`}
      >
        <ChevronDown size={15} aria-hidden="true" className="message-chevron" />
        <span className="message-avatar" aria-hidden="true">
          {sender.slice(0, 1).toUpperCase()}
        </span>
        <span className="conversation-sender">
          <strong>{sender}</strong>
          <span>{message.from_address}</span>
        </span>
      </button>
      {message.direction === "outbound" ? (
        <span className="delivery-status" data-status={message.status}>
          {message.status ?? "sent"}
        </span>
      ) : null}
      <MessageWarning message={message} />
      <time
        className="message-time"
        dateTime={messageTime(message) ?? undefined}
      >
        {mailDate(messageTime(message))}
      </time>
      {onReply ? (
        <button
          className="icon-button"
          type="button"
          onClick={onReply}
          title="Reply"
          aria-label="Reply"
        >
          <Reply size={17} aria-hidden="true" />
        </button>
      ) : null}
    </header>
  );
}

function MessageMetadata({ message }: { message: MailboxMessageDetail }) {
  const recipientGroups = (["to", "cc", "bcc"] as const)
    .map((kind) => ({
      kind,
      label: { to: "To", cc: "Cc", bcc: "Bcc" }[kind],
      addresses: message.recipients
        .filter((recipient) => recipient.kind === kind)
        .map((recipient) => recipient.address)
        .join(", "),
    }))
    .filter((group) => group.kind === "to" || group.addresses);
  return (
    <div className="conversation-meta">
      {recipientGroups.map((group) => (
        <span key={group.kind}>
          {group.label} {group.addresses || "undisclosed recipients"}
        </span>
      ))}
      {message.direction !== "outbound" ? (
        <SecurityDetails message={message} />
      ) : null}
    </div>
  );
}

function MessageWarning({ message }: { message: MailboxMessageDetail }) {
  if (message.direction === "outbound") return null;
  const clean =
    message.auth_verdict === "pass" &&
    message.spam_result === "pass" &&
    message.virus_result === "pass";
  return clean ? null : (
    <span
      className="auth-warning"
      title="Authentication or security scans need review"
    >
      Security warning
    </span>
  );
}

function MessageFiles({
  message,
  onDownloadAttachment,
}: Pick<MessageDetailProps, "message" | "onDownloadAttachment">) {
  if (!message.attachments.length) return null;
  return (
    <div className="message-files" aria-label="Attachments">
      {message.attachments.map((attachment) => (
        <button
          className="message-file"
          key={attachment.id}
          type="button"
          disabled={!onDownloadAttachment}
          aria-label={`Download ${sanitizeAttachmentName(attachment.display_filename || attachment.filename)}`}
          onClick={() =>
            onDownloadAttachment?.(message, attachment.id).catch(
              () => undefined,
            )
          }
        >
          <Paperclip size={15} aria-hidden="true" />
          <span>
            {sanitizeAttachmentName(
              attachment.display_filename || attachment.filename,
            )}
            <small>
              {formatBytes(attachment.size_bytes)} · {attachment.content_type}
            </small>
          </span>
        </button>
      ))}
    </div>
  );
}

function MessageActions({
  message,
  contacts = [],
  onToggleRead,
  onContactChange,
}: MessageDetailProps) {
  return (
    <div className="conversation-message-actions">
      {onToggleRead ? (
        <button
          type="button"
          className="text-button"
          onClick={() => onToggleRead(message)}
        >
          <Mail size={14} aria-hidden="true" />
          {message.is_read ? "Mark unread" : "Mark read"}
        </button>
      ) : null}
      {onContactChange ? (
        <select
          aria-label="Contact association"
          value={message.contact_id ?? ""}
          onChange={(event) =>
            onContactChange(message, event.currentTarget.value || null)
          }
        >
          <option value="">No contact</option>
          {contacts.map((contact) => (
            <option key={contact.id} value={contact.id}>
              {contact.display_name}
            </option>
          ))}
        </select>
      ) : null}
    </div>
  );
}

function SecurityDetails({ message }: { message: MailboxMessageDetail }) {
  const clean =
    message.spam_result === "pass" && message.virus_result === "pass";
  const authenticated = message.auth_verdict === "pass";
  const checks = [
    ["Auth verdict", message.auth_verdict],
    ["SPF", message.spf_result],
    ["DKIM", message.dkim_result],
    ["DMARC", message.dmarc_result],
    ["Spam", message.spam_result],
    ["Virus", message.virus_result],
    ["Security", message.security_disposition],
  ];
  return (
    <details
      className="security-details"
      data-warning={!clean || !authenticated}
    >
      <summary>
        <ShieldCheck size={13} aria-hidden="true" />
        {authenticated ? "Authenticated" : "Authentication needs review"} ·{" "}
        {clean ? "Scans passed" : "Review scans"}
      </summary>
      <dl>
        {checks.map(([label, value]) => (
          <div key={label}>
            <dt>{label}</dt>
            <dd>{value ?? "unknown"}</dd>
          </div>
        ))}
      </dl>
    </details>
  );
}
