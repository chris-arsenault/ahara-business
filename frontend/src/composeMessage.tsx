import { useEffect, useRef, useState, type FormEvent } from "react";
import { Minus, Send, X } from "lucide-react";
import type { MailboxApi } from "./mailboxTypes";
import {
  ComposerAttachments,
  useComposerAttachments,
} from "./composerAttachments";

type ComposeProps = {
  apiClient: MailboxApi;
  open: boolean;
  onClose: () => void;
  onQueued: () => void;
};
const emptyDraft = {
  fromAddress: "contact@ahara.io",
  to: "",
  cc: "",
  bcc: "",
  subject: "",
  bodyText: "",
};

function useComposeDraft({ apiClient, onClose, onQueued }: ComposeProps) {
  const [draft, setDraft] = useState(emptyDraft);
  const [error, setError] = useState<string>();
  const [sending, setSending] = useState(false);
  const files = useComposerAttachments(apiClient);
  function update(field: keyof typeof draft, value: string) {
    setDraft((current) => ({ ...current, [field]: value }));
  }
  function discard() {
    setDraft((current) => ({
      ...emptyDraft,
      fromAddress: current.fromAddress,
    }));
    files.clear();
    setError(undefined);
    onClose();
  }
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (sending || files.busy) return;
    setError(undefined);
    if (!apiClient.composeMessage) {
      setError("Compose is unavailable");
      return;
    }
    setSending(true);
    try {
      await apiClient.composeMessage({
        from_address: draft.fromAddress,
        to: addresses(draft.to),
        cc: addresses(draft.cc),
        bcc: addresses(draft.bcc),
        subject: draft.subject,
        body_text: draft.bodyText,
        attachments: files.references,
      });
      discard();
      onQueued();
    } catch (failure) {
      setError(
        failure instanceof Error ? failure.message : "Unable to send message",
      );
    } finally {
      setSending(false);
    }
  }
  return { draft, update, discard, submit, files, error, sending };
}
type ComposeModel = ReturnType<typeof useComposeDraft>;

export function ComposeMessage(props: ComposeProps) {
  const dialog = useRef<HTMLDialogElement>(null);
  const model = useComposeDraft(props);
  useEffect(() => {
    const element = dialog.current;
    if (!element) return;
    if (props.open) {
      if (element.showModal) {
        if (!element.open) element.showModal();
      } else element.setAttribute("open", "");
      element.querySelector<HTMLInputElement>('[aria-label="To"]')?.focus();
    } else if (element.close) element.close();
    else element.removeAttribute("open");
  }, [props.open]);
  return (
    <dialog
      ref={dialog}
      className="compose-dialog"
      aria-label="New message"
      onCancel={(event) => {
        event.preventDefault();
        if (!model.sending) props.onClose();
      }}
    >
      <form className="compose-message" onSubmit={model.submit}>
        <header className="compose-heading">
          <h2>New message</h2>
          <button
            type="button"
            className="icon-button"
            aria-label="Close composer"
            disabled={model.sending}
            onClick={props.onClose}
          >
            <Minus size={18} />
          </button>
        </header>
        <ComposeFields model={model} />
        <textarea
          className="compose-body-input"
          aria-label="Body"
          placeholder="Write your message…"
          value={model.draft.bodyText}
          disabled={model.sending}
          onChange={(event) =>
            model.update("bodyText", event.currentTarget.value)
          }
        />
        <ComposerAttachments model={model.files} disabled={model.sending} />
        <ComposeFooter model={model} />
        {model.error ? (
          <p className="compose-status" role="alert">
            {model.error}
          </p>
        ) : null}
      </form>
    </dialog>
  );
}

function ComposeFields({ model }: { model: ComposeModel }) {
  const [more, setMore] = useState(false);
  const fields = [
    { key: "fromAddress", label: "From" },
    { key: "to", label: "To" },
    ...(more
      ? [
          { key: "cc", label: "Cc" },
          { key: "bcc", label: "Bcc" },
        ]
      : []),
    { key: "subject", label: "Subject" },
  ] as { key: keyof typeof emptyDraft; label: string }[];
  return (
    <div className="compose-address-fields">
      {fields.map((field) => (
        <label key={field.key}>
          <span>{field.label}</span>
          <input
            aria-label={field.label}
            value={model.draft[field.key]}
            disabled={model.sending}
            onChange={(event) =>
              model.update(field.key, event.currentTarget.value)
            }
          />
          {field.key === "to" ? (
            <button
              className="text-button"
              type="button"
              aria-expanded={more}
              onClick={() => setMore((value) => !value)}
            >
              Cc / Bcc
            </button>
          ) : null}
        </label>
      ))}
    </div>
  );
}

function ComposeFooter({ model }: { model: ComposeModel }) {
  return (
    <footer className="compose-footer">
      <button
        className="primary-button"
        type="submit"
        disabled={
          model.sending ||
          model.files.busy ||
          !model.draft.to.trim() ||
          (!model.draft.bodyText.trim() && !model.files.files.length)
        }
      >
        <Send size={16} aria-hidden="true" />
        {model.sending ? "Sending…" : "Send"}
      </button>
      <button
        type="button"
        className="quiet-button"
        disabled={model.sending}
        onClick={model.discard}
      >
        <X size={15} aria-hidden="true" />
        Discard
      </button>
    </footer>
  );
}

function addresses(value: string) {
  return value
    .split(",")
    .map((address) => address.trim())
    .filter(Boolean);
}
