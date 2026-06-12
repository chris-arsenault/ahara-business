/* eslint-disable complexity, max-lines-per-function, react-hooks/exhaustive-deps */
import { useCallback, useEffect, useState, type FormEvent } from "react";
import {
  Inbox,
  Paperclip,
  PenLine,
  RefreshCw,
  Reply,
  Send,
  ShieldCheck,
  UserRound,
} from "lucide-react";
import type {
  DetailState,
  InboxMailboxContentProps,
  MailboxApi,
  MailboxListProps,
  MailboxMode,
  MailboxState,
  MailboxViewProps,
  MessageDetailProps,
  OutboundDetailState,
  SentMailboxContentProps,
  SentMailboxState,
  SentMessageListProps,
  ThreadDetailProps,
} from "./mailboxTypes";
import { formatBytes, sanitizeAttachmentName } from "./textRendering";
import type {
  Contact,
  MailboxAttachment,
  MailboxMessageDetail,
  MailboxMessageSummary,
  OutboundMessageDetail,
  OutboundMessageSummary,
  ReplyMessageRequest,
} from "./types";

export type { MailboxApi } from "./mailboxTypes";

export function MailboxView({ apiClient }: MailboxViewProps) {
  const [mailboxMode, setMailboxMode] = useState<MailboxMode>("inbox");
  const [state, setState] = useState<MailboxState>({ status: "loading" });
  const [detailState, setDetailState] = useState<DetailState>({
    status: "empty",
  });
  const [sentState, setSentState] = useState<SentMailboxState>({
    status: "idle",
  });
  const [outboundDetailState, setOutboundDetailState] =
    useState<OutboundDetailState>({
      status: "empty",
    });
  const [selectedMessageId, setSelectedMessageId] = useState<string>();
  const [selectedOutboundId, setSelectedOutboundId] = useState<string>();
  const [contacts, setContacts] = useState<Contact[]>([]);
  const [actionError, setActionError] = useState<string>();
  const [searchQuery, setSearchQuery] = useState("");
  const [composeOpen, setComposeOpen] = useState(false);
  const showInbox = useCallback(() => setMailboxMode("inbox"), []);
  const showSent = useCallback(() => setMailboxMode("sent"), []);
  const toggleCompose = useCallback(() => setComposeOpen((open) => !open), []);
  const refreshCurrentMailbox = useCallback(() => {
    if (mailboxMode === "sent") {
      void loadSentMessages();
      return;
    }
    void loadMessages();
  }, [apiClient, mailboxMode]);
  const handleComposeQueued = useCallback(() => {
    void loadMessages();
    if (mailboxMode === "sent") {
      void loadSentMessages();
    }
  }, [apiClient, mailboxMode]);
  const handleSelectOutbound = useCallback(
    (message: OutboundMessageSummary) => void loadOutboundDetail(message),
    [apiClient],
  );
  const handleChangeMessageContact = useCallback(
    (message: MailboxMessageDetail, contactId: string | null) =>
      void changeMessageContact(message, contactId),
    [apiClient],
  );
  const handleReply = useCallback(
    (message: MailboxMessageDetail, request: ReplyMessageRequest) =>
      replyToMessage(message, request),
    [apiClient],
  );
  const handleSelectMessage = useCallback(
    (message: MailboxMessageSummary) => void loadMessageDetail(message),
    [apiClient],
  );
  const handleToggleRead = useCallback(
    (message: MailboxMessageSummary | MailboxMessageDetail) =>
      void toggleRead(message),
    [apiClient],
  );

  async function loadMessages() {
    setState({ status: "loading" });
    try {
      setState({
        status: "ready",
        messages: await apiClient.fetchMailboxMessages(),
      });
    } catch (error) {
      setState({
        status: "error",
        message:
          error instanceof Error ? error.message : "Unable to load mailbox",
      });
    }
  }

  async function loadSentMessages() {
    if (!apiClient.listOutboundMessages) {
      setSentState({ status: "error", message: "Sent mail is unavailable" });
      return;
    }
    setSentState({ status: "loading" });
    setSelectedOutboundId(undefined);
    setOutboundDetailState({ status: "empty" });
    try {
      setSentState({
        status: "ready",
        messages: await apiClient.listOutboundMessages(),
      });
    } catch (error) {
      setSentState({
        status: "error",
        message:
          error instanceof Error ? error.message : "Unable to load sent mail",
      });
    }
  }

  useEffect(() => {
    void loadMessages();
    void loadContacts();
  }, [apiClient]);

  useEffect(() => {
    if (mailboxMode === "sent") {
      void loadSentMessages();
    }
  }, [mailboxMode, apiClient]);

  return (
    <section className="mailbox-panel" aria-labelledby="mailbox-title">
      <header className="mailbox-toolbar">
        <div className="toolbar-title">
          <Inbox aria-hidden="true" size={18} />
          <h1 id="mailbox-title">Mailbox</h1>
        </div>
        <div className="mailbox-view-toggle" aria-label="Mailbox view">
          <button
            className="view-toggle-button"
            data-active={mailboxMode === "inbox"}
            type="button"
            aria-pressed={mailboxMode === "inbox"}
            onClick={showInbox}
          >
            <Inbox aria-hidden="true" size={15} />
            Inbox
          </button>
          <button
            className="view-toggle-button"
            data-active={mailboxMode === "sent"}
            type="button"
            aria-pressed={mailboxMode === "sent"}
            onClick={showSent}
          >
            <Send aria-hidden="true" size={15} />
            Sent
          </button>
        </div>
        {mailboxMode === "inbox" ? (
          <form
            className="mailbox-search"
            onSubmit={(event) => void submitSearch(event)}
          >
            <label>
              <span>Search mailbox</span>
              <input
                value={searchQuery}
                onChange={(event) => setSearchQuery(event.currentTarget.value)}
                placeholder="Search mailbox"
              />
            </label>
            <button className="secondary-button" type="submit">
              Search
            </button>
          </form>
        ) : null}
        <button
          className="icon-button"
          type="button"
          onClick={refreshCurrentMailbox}
          title={
            mailboxMode === "sent" ? "Refresh sent mail" : "Refresh mailbox"
          }
          aria-label={
            mailboxMode === "sent" ? "Refresh sent mail" : "Refresh mailbox"
          }
        >
          <RefreshCw aria-hidden="true" size={18} />
        </button>
        <button
          className="secondary-button"
          type="button"
          onClick={toggleCompose}
        >
          <PenLine aria-hidden="true" size={16} />
          Compose
        </button>
      </header>

      {composeOpen ? (
        <ComposeMessage apiClient={apiClient} onQueued={handleComposeQueued} />
      ) : null}
      {mailboxMode === "sent" ? (
        <SentMailboxContent
          onSelectOutbound={handleSelectOutbound}
          outboundDetailState={outboundDetailState}
          selectedOutboundId={selectedOutboundId}
          sentState={sentState}
        />
      ) : (
        <InboxMailboxContent
          contacts={contacts}
          detailState={detailState}
          onChangeMessageContact={handleChangeMessageContact}
          onReply={handleReply}
          onSelectMessage={handleSelectMessage}
          onToggleRead={handleToggleRead}
          selectedMessageId={selectedMessageId}
          state={state}
        />
      )}
      {actionError ? (
        <div className="error-state compact-error" role="alert">
          {actionError}
        </div>
      ) : null}
    </section>
  );

  async function loadContacts() {
    if (!apiClient.listContacts) {
      return;
    }
    try {
      setContacts(await apiClient.listContacts());
    } catch {
      setContacts([]);
    }
  }

  async function submitSearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const query = searchQuery.trim();
    setSelectedMessageId(undefined);
    setDetailState({ status: "empty" });
    if (!query) {
      await loadMessages();
      return;
    }
    if (!apiClient.searchMessages) {
      setState({ status: "error", message: "Search is unavailable" });
      return;
    }
    setState({ status: "loading" });
    try {
      setState({
        status: "ready",
        messages: await apiClient.searchMessages(query),
      });
    } catch (error) {
      setState({
        status: "error",
        message:
          error instanceof Error ? error.message : "Unable to search mailbox",
      });
    }
  }

  async function loadMessageDetail(message: MailboxMessageSummary) {
    setSelectedMessageId(message.id);
    setDetailState({ status: "loading" });
    try {
      if (message.thread_id && apiClient.fetchThreadDetail) {
        setDetailState({
          status: "ready",
          thread: await apiClient.fetchThreadDetail(message.thread_id),
        });
        return;
      }
      if (apiClient.fetchMessageDetail) {
        const detail = await apiClient.fetchMessageDetail(message.id);
        setDetailState({
          status: "ready",
          thread: {
            thread_id: detail.thread_id ?? detail.id,
            normalized_subject: detail.subject.toLowerCase(),
            message_count: 1,
            last_activity_at: detail.received_at,
            messages: [detail],
          },
        });
        return;
      }
      throw new Error("Message detail is unavailable");
    } catch (error) {
      setDetailState({
        status: "error",
        message:
          error instanceof Error ? error.message : "Unable to load message",
      });
    }
  }

  async function loadOutboundDetail(message: OutboundMessageSummary) {
    if (!apiClient.fetchOutboundMessage) {
      setOutboundDetailState({
        status: "error",
        message: "Sent message detail is unavailable",
      });
      return;
    }
    setSelectedOutboundId(message.id);
    setOutboundDetailState({ status: "loading" });
    try {
      setOutboundDetailState({
        status: "ready",
        message: await apiClient.fetchOutboundMessage(message.id),
      });
    } catch (error) {
      setOutboundDetailState({
        status: "error",
        message:
          error instanceof Error
            ? error.message
            : "Unable to load sent message",
      });
    }
  }

  async function toggleRead(
    message: MailboxMessageSummary | MailboxMessageDetail,
  ) {
    if (!apiClient.updateMessageState) {
      setActionError("Read state is unavailable");
      return;
    }
    setActionError(undefined);
    try {
      const updated = await apiClient.updateMessageState(
        message.id,
        !message.is_read,
      );
      applySummaryUpdate(updated);
    } catch (error) {
      setActionError(
        error instanceof Error ? error.message : "Unable to update message",
      );
    }
  }

  async function changeMessageContact(
    message: MailboxMessageDetail,
    contactId: string | null,
  ) {
    if (!apiClient.linkMessageContact) {
      setActionError("Contact association is unavailable");
      return;
    }
    setActionError(undefined);
    try {
      const updated = await apiClient.linkMessageContact(message.id, contactId);
      applySummaryUpdate(updated);
    } catch (error) {
      setActionError(
        error instanceof Error ? error.message : "Unable to update contact",
      );
    }
  }

  async function replyToMessage(
    message: MailboxMessageDetail,
    request: ReplyMessageRequest,
  ) {
    if (!apiClient.replyToMessage) {
      throw new Error("Reply is unavailable");
    }
    await apiClient.replyToMessage(message.id, request);
  }

  function applySummaryUpdate(updated: MailboxMessageSummary) {
    setState((current) =>
      current.status === "ready"
        ? {
            ...current,
            messages: current.messages.map((message) =>
              message.id === updated.id ? updated : message,
            ),
          }
        : current,
    );
    setDetailState((current) =>
      current.status === "ready"
        ? {
            ...current,
            thread: {
              ...current.thread,
              messages: current.thread.messages.map((message) =>
                message.id === updated.id
                  ? {
                      ...message,
                      is_read: updated.is_read,
                      contact_id: updated.contact_id,
                    }
                  : message,
              ),
            },
          }
        : current,
    );
  }
}

function InboxMailboxContent({
  contacts,
  detailState,
  onChangeMessageContact,
  onReply,
  onSelectMessage,
  onToggleRead,
  selectedMessageId,
  state,
}: InboxMailboxContentProps) {
  if (state.status === "loading") {
    return (
      <div className="empty-state" role="status">
        Loading mailbox
      </div>
    );
  }
  if (state.status === "error") {
    return (
      <div className="error-state" role="alert">
        {state.message}
      </div>
    );
  }

  return (
    <div className="mailbox-content">
      <MailboxList
        messages={state.messages}
        selectedMessageId={selectedMessageId}
        onSelectMessage={onSelectMessage}
        onToggleRead={onToggleRead}
      />
      <DetailPane
        state={detailState}
        contacts={contacts}
        onToggleRead={onToggleRead}
        onContactChange={onChangeMessageContact}
        onReply={onReply}
      />
    </div>
  );
}

function SentMailboxContent({
  onSelectOutbound,
  outboundDetailState,
  selectedOutboundId,
  sentState,
}: SentMailboxContentProps) {
  if (sentState.status === "idle" || sentState.status === "loading") {
    return (
      <div className="empty-state" role="status">
        Loading sent mail
      </div>
    );
  }
  if (sentState.status === "error") {
    return (
      <div className="error-state" role="alert">
        {sentState.message}
      </div>
    );
  }

  return (
    <div className="mailbox-content">
      <SentMessageList
        messages={sentState.messages}
        selectedMessageId={selectedOutboundId}
        onSelectMessage={onSelectOutbound}
      />
      <OutboundDetailPane state={outboundDetailState} />
    </div>
  );
}

export function MailboxList({
  messages,
  selectedMessageId,
  onSelectMessage,
  onToggleRead,
}: MailboxListProps) {
  if (messages.length === 0) {
    return <div className="empty-state">No accepted messages</div>;
  }

  return (
    <ol className="mailbox-list" aria-label="Mailbox messages">
      {messages.map((message) => (
        <li
          className="mailbox-row"
          data-selected={selectedMessageId === message.id}
          key={message.id}
        >
          <button
            className="mailbox-row-main"
            type="button"
            onClick={() => onSelectMessage?.(message)}
          >
            <span className="unread-cell">
              {!message.is_read ? (
                <span className="unread-dot" aria-label="Unread message" />
              ) : null}
            </span>
            <span className="sender-cell">
              <span className="sender-address">{message.from_address}</span>
              {message.from_display_name ? (
                <span className="sender-display">
                  <UserRound aria-hidden="true" size={13} />
                  {message.from_display_name}
                </span>
              ) : null}
            </span>
            <span className="message-cell">
              <span className="message-subject">
                {message.subject || "(no subject)"}
              </span>
              <span className="message-snippet">{message.snippet}</span>
            </span>
            <span className="meta-cell">
              <span className="verdict-pill">
                <ShieldCheck aria-hidden="true" size={13} />
                {message.auth_verdict ?? "unknown"}
              </span>
              {message.attachment_count > 0 ? (
                <span
                  className="attachment-count"
                  aria-label="Attachment count"
                >
                  <Paperclip aria-hidden="true" size={13} />
                  {message.attachment_count}
                </span>
              ) : null}
            </span>
          </button>
          {onToggleRead ? (
            <button
              className="icon-button row-action"
              type="button"
              onClick={() => onToggleRead(message)}
              title={message.is_read ? "Mark unread" : "Mark read"}
              aria-label={message.is_read ? "Mark unread" : "Mark read"}
            >
              <Inbox aria-hidden="true" size={16} />
            </button>
          ) : null}
        </li>
      ))}
    </ol>
  );
}

function SentMessageList({
  messages,
  selectedMessageId,
  onSelectMessage,
}: SentMessageListProps) {
  if (messages.length === 0) {
    return <div className="empty-state">No sent messages</div>;
  }

  return (
    <ol className="mailbox-list sent-message-list" aria-label="Sent messages">
      {messages.map((message) => (
        <li
          className="mailbox-row sent-row"
          data-selected={selectedMessageId === message.id}
          key={message.id}
        >
          <button
            className="mailbox-row-main sent-row-main"
            type="button"
            onClick={() => onSelectMessage?.(message)}
          >
            <span className="sent-cell">
              <Send aria-hidden="true" size={14} />
            </span>
            <span className="sender-cell">
              <span className="sender-address">
                {recipientSummary(message)}
              </span>
              <span className="sender-display">
                From {message.from_address}
              </span>
            </span>
            <span className="message-cell">
              <span className="message-subject">
                {message.subject || "(no subject)"}
              </span>
              <span
                className="message-snippet"
                data-error={message.last_error ? "true" : "false"}
              >
                {message.last_error ?? message.snippet}
              </span>
            </span>
            <span className="meta-cell">
              <span className="verdict-pill status-pill">{message.status}</span>
              <span className="sent-timestamp">
                {displayDateTime(message.sent_at ?? message.created_at)}
              </span>
            </span>
          </button>
        </li>
      ))}
    </ol>
  );
}

function DetailPane({
  state,
  contacts,
  onToggleRead,
  onContactChange,
  onReply,
}: {
  state: DetailState;
  contacts: Contact[];
  onToggleRead: (message: MailboxMessageDetail) => void;
  onContactChange: (
    message: MailboxMessageDetail,
    contactId: string | null,
  ) => void;
  onReply: (
    message: MailboxMessageDetail,
    request: ReplyMessageRequest,
  ) => Promise<void>;
}) {
  if (state.status === "empty") {
    return <div className="detail-empty">Select a message</div>;
  }
  if (state.status === "loading") {
    return (
      <div className="detail-empty" role="status">
        Loading message
      </div>
    );
  }
  if (state.status === "error") {
    return (
      <div className="error-state" role="alert">
        {state.message}
      </div>
    );
  }
  return (
    <ThreadDetail
      thread={state.thread}
      contacts={contacts}
      onToggleRead={onToggleRead}
      onContactChange={onContactChange}
      onReply={onReply}
    />
  );
}

function OutboundDetailPane({ state }: { state: OutboundDetailState }) {
  if (state.status === "empty") {
    return <div className="detail-empty">Select a sent message</div>;
  }
  if (state.status === "loading") {
    return (
      <div className="detail-empty" role="status">
        Loading sent message
      </div>
    );
  }
  if (state.status === "error") {
    return (
      <div className="error-state" role="alert">
        {state.message}
      </div>
    );
  }
  return <OutboundMessageDetailView message={state.message} />;
}

function OutboundMessageDetailView({
  message,
}: {
  message: OutboundMessageDetail;
}) {
  return (
    <section className="thread-detail" aria-label="Sent message detail">
      <article className="message-detail sent-detail">
        <header className="message-detail-header">
          <div className="message-from-block">
            <span className="message-from-address">
              {message.subject || "(no subject)"}
            </span>
            <span className="message-from-display">
              {message.status} · {displayDateTime(message.sent_at)}
            </span>
          </div>
          <div className="message-security-grid outbound-address-grid">
            <SecurityLine label="Status" value={message.status} />
            <SecurityLine label="From" value={message.from_address} />
            <SecurityLine
              label="To"
              value={recipientAddresses(message, "to")}
            />
            <SecurityLine
              label="Cc"
              value={recipientAddresses(message, "cc")}
            />
            <SecurityLine
              label="Bcc"
              value={recipientAddresses(message, "bcc")}
            />
            <SecurityLine
              label="Created"
              value={displayDateTime(message.created_at)}
            />
          </div>
          {message.last_error ? (
            <div className="error-state compact-error" role="alert">
              {message.last_error}
            </div>
          ) : null}
        </header>
        <h2 className="message-detail-subject">
          {message.subject || "(no subject)"}
        </h2>
        <pre className="message-body-text">{message.body_text}</pre>
      </article>
    </section>
  );
}

export function ThreadDetail({
  thread,
  contacts = [],
  onToggleRead,
  onContactChange,
  onReply,
}: ThreadDetailProps) {
  return (
    <section className="thread-detail" aria-label="Thread detail">
      {thread.messages.map((message) => (
        <MessageDetail
          contacts={contacts}
          key={message.id}
          message={message}
          onContactChange={onContactChange}
          onReply={onReply}
          onToggleRead={onToggleRead}
        />
      ))}
    </section>
  );
}

export function MessageDetail({
  message,
  contacts = [],
  onToggleRead,
  onContactChange,
  onReply,
}: MessageDetailProps) {
  const [replyOpen, setReplyOpen] = useState(false);
  const closeReply = useCallback(() => setReplyOpen(false), []);
  return (
    <article className="message-detail" aria-label="Message detail">
      <header className="message-detail-header">
        <div className="message-from-block">
          <span className="message-from-address">{message.from_address}</span>
          {message.from_display_name ? (
            <span className="message-from-display">
              {message.from_display_name}
            </span>
          ) : null}
        </div>
        <div className="message-security-grid">
          <SecurityLine label="Auth verdict" value={message.auth_verdict} />
          <SecurityLine label="SPF" value={message.spf_result} />
          <SecurityLine label="DKIM" value={message.dkim_result} />
          <SecurityLine label="DMARC" value={message.dmarc_result} />
          <SecurityLine label="Spam" value={message.spam_result} />
          <SecurityLine label="Virus" value={message.virus_result} />
          <SecurityLine label="Security" value={message.security_disposition} />
        </div>
        <div className="message-actions">
          {onToggleRead ? (
            <button
              className="secondary-button"
              type="button"
              onClick={() => onToggleRead(message)}
            >
              {message.is_read ? "Mark unread" : "Mark read"}
            </button>
          ) : null}
          {onContactChange ? (
            <label className="contact-control">
              <span>Contact association</span>
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
            </label>
          ) : null}
          {onReply ? (
            <button
              className="secondary-button"
              type="button"
              onClick={() => setReplyOpen((open) => !open)}
            >
              <Reply aria-hidden="true" size={16} />
              Reply
            </button>
          ) : null}
        </div>
      </header>
      <h2 className="message-detail-subject">
        {message.subject || "(no subject)"}
      </h2>
      <pre className="message-body-text">{message.body_text}</pre>
      {replyOpen && onReply ? (
        <ReplyMessageForm
          message={message}
          onReply={onReply}
          onQueued={closeReply}
        />
      ) : null}
      {message.attachments.length > 0 ? (
        <AttachmentList attachments={message.attachments} />
      ) : null}
    </article>
  );
}

function ComposeMessage({
  apiClient,
  onQueued,
}: {
  apiClient: MailboxApi;
  onQueued: () => void;
}) {
  const [fromAddress, setFromAddress] = useState("contact@ahara.io");
  const [to, setTo] = useState("");
  const [subject, setSubject] = useState("");
  const [bodyText, setBodyText] = useState("");
  const [status, setStatus] = useState<string>();

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setStatus(undefined);
    if (!apiClient.composeMessage) {
      setStatus("Compose is unavailable");
      return;
    }
    try {
      await apiClient.composeMessage({
        from_address: fromAddress,
        to: splitAddresses(to),
        cc: [],
        bcc: [],
        subject,
        body_text: bodyText,
      });
      setSubject("");
      setBodyText("");
      setTo("");
      setStatus("Queued");
      onQueued();
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Unable to queue");
    }
  }

  return (
    <form className="compose-message" onSubmit={(event) => void submit(event)}>
      <label>
        <span>From</span>
        <input
          value={fromAddress}
          onChange={(event) => setFromAddress(event.currentTarget.value)}
        />
      </label>
      <label>
        <span>To</span>
        <input
          value={to}
          onChange={(event) => setTo(event.currentTarget.value)}
        />
      </label>
      <label>
        <span>Subject</span>
        <input
          value={subject}
          onChange={(event) => setSubject(event.currentTarget.value)}
        />
      </label>
      <label className="compose-body">
        <span>Body</span>
        <textarea
          value={bodyText}
          onChange={(event) => setBodyText(event.currentTarget.value)}
        />
      </label>
      <button className="primary-button" type="submit">
        <Send aria-hidden="true" size={16} />
        Send
      </button>
      {status ? <span className="compose-status">{status}</span> : null}
    </form>
  );
}

function ReplyMessageForm({
  message,
  onReply,
  onQueued,
}: {
  message: MailboxMessageDetail;
  onReply: (
    message: MailboxMessageDetail,
    request: ReplyMessageRequest,
  ) => Promise<void>;
  onQueued: () => void;
}) {
  const [fromAddress, setFromAddress] = useState("contact@ahara.io");
  const [bodyText, setBodyText] = useState("");
  const [status, setStatus] = useState<string>();

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setStatus(undefined);
    try {
      await onReply(message, {
        from_address: fromAddress,
        body_text: bodyText,
      });
      setBodyText("");
      setStatus("Queued");
      onQueued();
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Unable to queue");
    }
  }

  return (
    <form className="reply-message" onSubmit={(event) => void submit(event)}>
      <label>
        <span>From</span>
        <input
          value={fromAddress}
          onChange={(event) => setFromAddress(event.currentTarget.value)}
        />
      </label>
      <label className="compose-body">
        <span>Reply</span>
        <textarea
          value={bodyText}
          onChange={(event) => setBodyText(event.currentTarget.value)}
        />
      </label>
      <button className="primary-button" type="submit">
        <Send aria-hidden="true" size={16} />
        Send reply
      </button>
      {status ? <span className="compose-status">{status}</span> : null}
    </form>
  );
}

function splitAddresses(value: string) {
  return value
    .split(",")
    .map((address) => address.trim())
    .filter(Boolean);
}

function recipientSummary(message: OutboundMessageSummary) {
  const primary = message.primary_recipient ?? "No recipients";
  if (message.recipient_count <= 1) {
    return primary;
  }
  return `${primary} +${message.recipient_count - 1}`;
}

function recipientAddresses(
  message: OutboundMessageDetail,
  kind: "to" | "cc" | "bcc",
) {
  const addresses = message.recipients
    .filter((recipient) => recipient.kind === kind)
    .map((recipient) => recipient.address);
  return addresses.length > 0 ? addresses.join(", ") : "none";
}

function displayDateTime(value: string | null) {
  if (!value) {
    return "pending";
  }
  return value.replace("T", " ").split(".")[0];
}

function SecurityLine({
  label,
  value,
}: {
  label: string;
  value: string | null;
}) {
  return (
    <div className="security-line">
      <span>{label}</span>
      <strong>{value ?? "unknown"}</strong>
    </div>
  );
}

function AttachmentList({ attachments }: { attachments: MailboxAttachment[] }) {
  return (
    <section className="attachment-list" aria-label="Attachments">
      {attachments.map((attachment) => (
        <div className="attachment-item" key={attachment.id}>
          <Paperclip aria-hidden="true" size={15} />
          <span>
            {sanitizeAttachmentName(
              attachment.display_filename || attachment.filename,
            )}
          </span>
          <small>
            {attachment.content_type} · {formatBytes(attachment.size_bytes)}
          </small>
        </div>
      ))}
    </section>
  );
}
