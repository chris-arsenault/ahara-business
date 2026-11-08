/* eslint-disable complexity, max-lines-per-function, react-hooks/exhaustive-deps */
import { useCallback, useEffect, useState, type FormEvent } from "react";
import {
  ArrowLeft,
  Inbox,
  Paperclip,
  PenLine,
  RefreshCw,
  Send,
  ShieldCheck,
} from "lucide-react";
import {
  saveAttachmentDownload,
  type AttachmentUploadApi,
} from "./attachmentFiles";
import { ComposeMessage } from "./composeMessage";
import type {
  DetailState,
  InboxMailboxContentProps,
  MailboxListProps,
  MailboxMode,
  MailboxState,
  MailboxViewProps,
  OutboundDetailState,
  SentMailboxContentProps,
  SentMailboxState,
  SentMessageListProps,
  ThreadDetailProps,
} from "./mailboxTypes";
import { ThreadDetail } from "./conversation";
import {
  conversationRows,
  sentConversationRows,
  standaloneOutboundThread,
  mailDate,
  updateThreadSummary,
} from "./mailboxPresentation";
import "./mailbox.css";
import type {
  Contact,
  MailboxMessageDetail,
  MailboxMessageSummary,
  OutboundMessageSummary,
  ReplyMessageRequest,
} from "./types";

export type { MailboxApi } from "./mailboxTypes";
export { MessageDetail, ThreadDetail } from "./conversation";

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
  const [notice, setNotice] = useState<string>();
  const [searchQuery, setSearchQuery] = useState("");
  const [composeOpen, setComposeOpen] = useState(false);
  const showInbox = useCallback(() => setMailboxMode("inbox"), []);
  const showSent = useCallback(() => setMailboxMode("sent"), []);
  const toggleCompose = useCallback(() => setComposeOpen((open) => !open), []);
  const closeReader = useCallback(() => {
    setDetailState({ status: "empty" });
    setOutboundDetailState({ status: "empty" });
    setSelectedMessageId(undefined);
    setSelectedOutboundId(undefined);
  }, []);
  const refreshCurrentMailbox = useCallback(() => {
    if (mailboxMode === "sent") {
      void loadSentMessages();
      if (
        outboundDetailState.status === "ready" &&
        outboundDetailState.message.thread_id
      ) {
        void refreshOpenThread(outboundDetailState.message.thread_id);
      }
      return;
    }
    void loadMessages();
    if (detailState.status === "ready")
      void refreshOpenThread(detailState.thread.thread_id);
  }, [apiClient, mailboxMode, detailState, outboundDetailState]);
  const handleComposeQueued = useCallback(() => {
    setComposeOpen(false);
    setNotice("Message queued for delivery. Track its status in Sent.");
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
  const handleDownloadAttachment = useCallback(
    (message: MailboxMessageDetail, attachmentId: string) =>
      downloadAttachment(message, attachmentId),
    [apiClient],
  );
  const handleSelectMessage = useCallback(
    (message: MailboxMessageSummary) => void loadMessageDetail(message),
    [apiClient],
  );
  const handleToggleRead = useCallback(
    (message: MailboxMessageSummary | MailboxMessageDetail) =>
      void toggleRead(message),
    [apiClient, state],
  );

  async function loadMessages() {
    setState((current) =>
      current.status === "ready" ? current : { status: "loading" },
    );
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
    setSentState((current) =>
      current.status === "ready" ? current : { status: "loading" },
    );
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

      {notice ? (
        <p className="mailbox-notice" role="status">
          {notice}
        </p>
      ) : null}

      <ComposeMessage
        apiClient={apiClient}
        open={composeOpen}
        onClose={() => setComposeOpen(false)}
        onQueued={handleComposeQueued}
      />
      {mailboxMode === "sent" ? (
        <SentMailboxContent
          uploadApi={apiClient}
          onBack={closeReader}
          onSelectOutbound={handleSelectOutbound}
          outboundDetailState={outboundDetailState}
          selectedOutboundId={selectedOutboundId}
          sentState={sentState}
          contacts={contacts}
          onReply={handleReply}
          onToggleRead={handleToggleRead}
          onContactChange={handleChangeMessageContact}
          onDownloadAttachment={handleDownloadAttachment}
        />
      ) : (
        <InboxMailboxContent
          uploadApi={apiClient}
          onBack={closeReader}
          contacts={contacts}
          detailState={detailState}
          onChangeMessageContact={handleChangeMessageContact}
          onDownloadAttachment={handleDownloadAttachment}
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
      const detail = await apiClient.fetchOutboundMessage(message.id);
      setOutboundDetailState({
        status: "ready",
        message: detail,
        thread: null,
      });
      if (detail.thread_id && apiClient.fetchThreadDetail) {
        try {
          const thread = await apiClient.fetchThreadDetail(detail.thread_id);
          setOutboundDetailState({ status: "ready", message: detail, thread });
        } catch {
          setActionError(
            "Sent message loaded. The rest of the conversation could not be loaded.",
          );
        }
      }
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
      const targets =
        "body_text" in message || !message.thread_id || state.status !== "ready"
          ? [message]
          : state.messages.filter(
              (item) => item.thread_id === message.thread_id,
            );
      await Promise.all(
        targets.map(async (item) => {
          const updated = await apiClient.updateMessageState!(
            item.id,
            !message.is_read,
          );
          applySummaryUpdate(updated);
        }),
      );
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
    if (message.thread_id && apiClient.fetchThreadDetail) {
      try {
        const thread = await apiClient.fetchThreadDetail(message.thread_id);
        setDetailState((current) =>
          current.status === "ready" &&
          current.thread.thread_id === message.thread_id
            ? { ...current, thread }
            : current,
        );
        setOutboundDetailState((current) =>
          current.status === "ready" &&
          current.message.thread_id === message.thread_id
            ? { ...current, thread }
            : current,
        );
      } catch {
        setActionError(
          "Your reply was queued. Refresh to load the updated conversation.",
        );
      }
    }
    void loadSentMessages();
  }

  async function refreshOpenThread(threadId: string) {
    if (!apiClient.fetchThreadDetail) return;
    try {
      const thread = await apiClient.fetchThreadDetail(threadId);
      setDetailState((current) =>
        current.status === "ready" && current.thread.thread_id === threadId
          ? { ...current, thread }
          : current,
      );
      setOutboundDetailState((current) =>
        current.status === "ready" && current.message.thread_id === threadId
          ? { ...current, thread }
          : current,
      );
    } catch {
      setActionError("Unable to refresh the conversation. Try again.");
    }
  }

  async function downloadAttachment(
    message: MailboxMessageDetail,
    attachmentId: string,
  ) {
    if (message.direction !== "outbound" && !apiClient.downloadAttachment) {
      setActionError("Attachment download is unavailable");
      return;
    }
    setActionError(undefined);
    try {
      if (message.direction === "outbound") {
        if (!apiClient.downloadOutboundAttachment)
          throw new Error("Attachment download is unavailable");
        const attachment = message.attachments.find(
          (item) => item.id === attachmentId,
        );
        if (!attachment) throw new Error("Attachment was not found");
        saveAttachmentDownload(
          await apiClient.downloadOutboundAttachment(
            message.id,
            attachment.position,
          ),
        );
        return;
      }
      saveAttachmentDownload(
        await apiClient.downloadAttachment!(message.id, attachmentId),
      );
    } catch (error) {
      setActionError(
        error instanceof Error
          ? error.message
          : "Unable to download attachment",
      );
    }
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
            thread: updateThreadSummary(current.thread, updated),
          }
        : current,
    );
    setOutboundDetailState((current) =>
      current.status === "ready" && current.thread
        ? { ...current, thread: updateThreadSummary(current.thread, updated) }
        : current,
    );
  }
}

function InboxMailboxContent({
  uploadApi,
  onBack,
  contacts,
  detailState,
  onChangeMessageContact,
  onDownloadAttachment,
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
    <div
      className="mailbox-content"
      data-reading={detailState.status !== "empty"}
    >
      <MailboxList
        messages={state.messages}
        selectedMessageId={selectedMessageId}
        onSelectMessage={onSelectMessage}
        onToggleRead={onToggleRead}
      />
      <div className="mailbox-reader">
        <button
          type="button"
          className="reader-back text-button"
          onClick={onBack}
        >
          <ArrowLeft size={16} aria-hidden="true" />
          Back to inbox
        </button>
        <DetailPane
          uploadApi={uploadApi}
          state={detailState}
          contacts={contacts}
          onToggleRead={onToggleRead}
          onContactChange={onChangeMessageContact}
          onDownloadAttachment={onDownloadAttachment}
          onReply={onReply}
        />
      </div>
    </div>
  );
}

function SentMailboxContent({
  onBack,
  onSelectOutbound,
  outboundDetailState,
  selectedOutboundId,
  sentState,
  ...threadActions
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
    <div
      className="mailbox-content"
      data-reading={outboundDetailState.status !== "empty"}
    >
      <SentMessageList
        messages={sentState.messages}
        selectedMessageId={selectedOutboundId}
        onSelectMessage={onSelectOutbound}
      />
      <div className="mailbox-reader">
        <button
          type="button"
          className="reader-back text-button"
          onClick={onBack}
        >
          <ArrowLeft size={16} aria-hidden="true" />
          Back to sent mail
        </button>
        <OutboundDetailPane state={outboundDetailState} {...threadActions} />
      </div>
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
  const selectedThreadId = messages.find(
    (message) => message.id === selectedMessageId,
  )?.thread_id;

  return (
    <ol className="mailbox-list" aria-label="Mailbox messages">
      {conversationRows(messages).map(({ message, count }) => (
        <li
          className="mailbox-row"
          data-unread={!message.is_read}
          data-selected={
            selectedMessageId === message.id ||
            Boolean(selectedThreadId && selectedThreadId === message.thread_id)
          }
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
              <span className="sender-display">
                {message.from_display_name || message.from_address}
                {count > 1 ? (
                  <span
                    className="conversation-count"
                    aria-label={`${count} received messages`}
                  >
                    {count}
                  </span>
                ) : null}
              </span>
            </span>
            <time
              className="list-timestamp"
              dateTime={message.received_at ?? undefined}
            >
              {mailDate(message.received_at, true)}
            </time>
            <span className="message-cell">
              <span className="message-subject">
                {message.subject || "(no subject)"}
              </span>
              <span className="message-snippet">{message.snippet}</span>
            </span>
            <span className="meta-cell">
              {message.auth_verdict && message.auth_verdict !== "pass" ? (
                <span
                  className="auth-warning"
                  title="Authentication needs review"
                >
                  <ShieldCheck aria-hidden="true" size={13} />
                  Review
                </span>
              ) : null}
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
      {sentConversationRows(messages).map(({ message, count }) => (
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
                {count > 1 ? (
                  <span
                    className="thread-count"
                    aria-label={`${count} sent messages`}
                  >
                    {count}
                  </span>
                ) : null}
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
  uploadApi,
  state,
  contacts,
  onToggleRead,
  onContactChange,
  onDownloadAttachment,
  onReply,
}: {
  uploadApi: AttachmentUploadApi;
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
  onDownloadAttachment: (
    message: MailboxMessageDetail,
    attachmentId: string,
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
      uploadApi={uploadApi}
      thread={state.thread}
      contacts={contacts}
      onToggleRead={onToggleRead}
      onContactChange={onContactChange}
      onDownloadAttachment={onDownloadAttachment}
      onReply={onReply}
    />
  );
}

function OutboundDetailPane({
  state,
  ...actions
}: { state: OutboundDetailState } & Omit<ThreadDetailProps, "thread">) {
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
  return (
    <>
      {state.message.last_error ? (
        <div className="error-state compact-error" role="alert">
          {state.message.last_error}
        </div>
      ) : null}
      <ThreadDetail
        thread={state.thread ?? standaloneOutboundThread(state.message)}
        {...actions}
      />
    </>
  );
}

function recipientSummary(message: OutboundMessageSummary) {
  const primary = message.primary_recipient ?? "No recipients";
  if (message.recipient_count <= 1) {
    return primary;
  }
  return `${primary} +${message.recipient_count - 1}`;
}

function displayDateTime(value: string | null) {
  if (!value) {
    return "pending";
  }
  return value.replace("T", " ").split(".")[0];
}
