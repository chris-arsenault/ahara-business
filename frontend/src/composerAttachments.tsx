/* eslint-disable max-lines-per-function */
import { useEffect, useRef, useState } from "react";
import { Paperclip, X, LoaderCircle } from "lucide-react";
import {
  displayFileSize,
  MAX_ATTACHMENT_BYTES,
  MAX_ATTACHMENT_COUNT,
  uploadAttachment,
  type AttachmentUploadApi,
} from "./attachmentFiles";

type PendingFile = {
  id: number;
  file: File;
  controller: AbortController;
  status: "uploading" | "ready" | "error";
  upload_id?: string;
  error?: string;
};

export function useComposerAttachments(api: AttachmentUploadApi) {
  const [files, setFiles] = useState<PendingFile[]>([]);
  const [error, setError] = useState<string>();
  const active = useRef(new Map<number, AbortController>());
  const sequence = useRef(0);
  useEffect(() => {
    const controllers = active.current;
    return () => {
      controllers.forEach((controller) => controller.abort());
      controllers.clear();
    };
  }, []);
  function remove(id: number) {
    active.current.get(id)?.abort();
    active.current.delete(id);
    setFiles((current) => current.filter((file) => file.id !== id));
  }
  function clear() {
    active.current.forEach((controller) => controller.abort());
    active.current.clear();
    setFiles([]);
    setError(undefined);
  }
  function update(id: number, change: Partial<PendingFile>) {
    setFiles((current) =>
      current.map((file) => (file.id === id ? { ...file, ...change } : file)),
    );
  }
  async function start(entry: PendingFile) {
    active.current.set(entry.id, entry.controller);
    try {
      const upload_id = await uploadAttachment(
        api,
        entry.file,
        entry.controller.signal,
      );
      update(entry.id, { status: "ready", upload_id });
    } catch (failure) {
      if (!entry.controller.signal.aborted)
        update(entry.id, {
          status: "error",
          error: failure instanceof Error ? failure.message : "Upload failed",
        });
    } finally {
      active.current.delete(entry.id);
    }
  }
  function select(selected: FileList | null) {
    if (!selected?.length) return;
    setError(undefined);
    const incoming = Array.from(selected);
    const total = [...files.map((entry) => entry.file), ...incoming];
    const validation = validateFiles(total);
    if (validation) {
      setError(validation);
      return;
    }
    const additions = incoming.map(
      (file): PendingFile => ({
        id: sequence.current++,
        file,
        controller: new AbortController(),
        status: "uploading",
      }),
    );
    setFiles((current) => [...current, ...additions]);
    additions.forEach((entry) => {
      start(entry);
    });
  }
  return {
    files,
    error,
    select,
    remove,
    clear,
    busy: files.some((file) => file.status !== "ready"),
    references: files.flatMap((file) =>
      file.status === "ready" && file.upload_id
        ? [{ upload_id: file.upload_id }]
        : [],
    ),
  };
}

export function ComposerAttachments({
  model,
  disabled,
  label = "Attachments",
}: {
  model: ReturnType<typeof useComposerAttachments>;
  disabled: boolean;
  label?: string;
}) {
  return (
    <div className="composer-files">
      {model.files.length ? (
        <ul className="compose-attachment-list" aria-label="Attached files">
          {model.files.map((entry) => (
            <AttachmentChip
              key={entry.id}
              entry={entry}
              disabled={disabled}
              remove={model.remove}
            />
          ))}
        </ul>
      ) : null}
      <div className="attachment-controls">
        <label className="attachment-picker">
          <Paperclip size={16} aria-hidden="true" />
          <span>Attach files</span>
          <input
            aria-label={label}
            type="file"
            multiple
            disabled={disabled}
            onChange={(event) => {
              model.select(event.currentTarget.files);
              event.currentTarget.value = "";
            }}
          />
        </label>
        <small>
          15 MB total
          {model.files.length
            ? ` · ${displayFileSize(model.files.reduce((sum, entry) => sum + entry.file.size, 0))} selected`
            : ""}
        </small>
      </div>
      {model.error ? (
        <p role="alert" className="compose-status">
          {model.error}
        </p>
      ) : null}
    </div>
  );
}

function validateFiles(files: File[]) {
  if (
    files.length > MAX_ATTACHMENT_COUNT ||
    files.reduce((sum, file) => sum + file.size, 0) > MAX_ATTACHMENT_BYTES
  )
    return "Attach up to 10 files, 15 MB (15 MiB) combined.";
  if (files.some((file) => file.size === 0))
    return "Empty files cannot be attached.";
  return undefined;
}

function AttachmentChip({
  entry,
  disabled,
  remove,
}: {
  entry: PendingFile;
  disabled: boolean;
  remove: (id: number) => void;
}) {
  return (
    <li data-status={entry.status}>
      {entry.status === "uploading" ? (
        <LoaderCircle size={14} className="upload-spinner" aria-hidden="true" />
      ) : (
        <Paperclip size={14} aria-hidden="true" />
      )}
      <span className="attachment-filename">{entry.file.name}</span>
      <small>
        {displayFileSize(entry.file.size)}
        {entry.status === "uploading" ? " · Uploading…" : ""}
      </small>
      <button
        type="button"
        disabled={disabled}
        className="icon-button"
        aria-label={`Remove ${entry.file.name}`}
        onClick={() => remove(entry.id)}
      >
        <X size={14} />
      </button>
      {entry.error ? (
        <span role="alert" className="upload-error">
          {entry.error}
        </span>
      ) : null}
    </li>
  );
}
