/* eslint-disable max-lines-per-function */
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Send, X } from "lucide-react";
import {
  ComposerAttachments,
  useComposerAttachments,
} from "./composerAttachments";
import type { AttachmentUploadApi } from "./attachmentFiles";
import type { MessageDetailProps } from "./mailboxTypes";
import type { MailboxMessageDetail } from "./types";

export function ReplyForm({
  message,
  onReply,
  onQueued,
  uploadApi,
  onCancel,
}: {
  message: MailboxMessageDetail;
  onReply: NonNullable<MessageDetailProps["onReply"]>;
  onQueued: () => void;
  uploadApi: AttachmentUploadApi;
  onCancel: () => void;
}) {
  const bodyRef = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    bodyRef.current?.focus();
  }, []);
  const [fromAddress, setFromAddress] = useState(
    () =>
      message.recipients.find((recipient) =>
        recipient.address.toLowerCase().endsWith("@ahara.io"),
      )?.address ?? "contact@ahara.io",
  );
  const [bodyText, setBodyText] = useState("");
  const attachments = useComposerAttachments(uploadApi);
  const [error, setError] = useState<string>();
  const [sending, setSending] = useState(false);
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (attachments.busy || sending) return;
    setError(undefined);
    setSending(true);
    try {
      await onReply(message, {
        from_address: fromAddress,
        body_text: bodyText,
        attachments: attachments.references,
      });
      onQueued();
    } catch (failure) {
      setError(
        failure instanceof Error ? failure.message : "Unable to send reply",
      );
    } finally {
      setSending(false);
    }
  }
  return (
    <form
      className="reply-message"
      onSubmit={(event) => void submit(event)}
      aria-label="Reply composer"
    >
      <div className="reply-addresses">
        <span>To {message.from_address}</span>
        <label>
          <span>From</span>
          <input
            value={fromAddress}
            disabled={sending}
            onChange={(event) => setFromAddress(event.currentTarget.value)}
          />
        </label>
      </div>
      <label className="compose-body">
        <span className="visually-hidden">Reply</span>
        <textarea
          ref={bodyRef}
          disabled={sending}
          aria-label="Reply"
          placeholder="Write your reply…"
          value={bodyText}
          onChange={(event) => setBodyText(event.currentTarget.value)}
        />
      </label>
      <ComposerAttachments
        model={attachments}
        disabled={sending}
        label="Reply attachments"
      />
      <div className="reply-footer">
        <button
          className="primary-button"
          type="submit"
          disabled={
            attachments.busy ||
            sending ||
            (!bodyText.trim() && !attachments.files.length)
          }
        >
          <Send size={15} aria-hidden="true" />
          {sending ? "Sending…" : "Send reply"}
        </button>
        <button
          className="quiet-button"
          type="button"
          disabled={sending}
          onClick={onCancel}
        >
          <X size={15} aria-hidden="true" />
          Discard reply
        </button>
      </div>
      {error ? (
        <p className="compose-status" role="alert">
          {error}
        </p>
      ) : null}
    </form>
  );
}
