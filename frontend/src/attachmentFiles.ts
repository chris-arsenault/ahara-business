import type { AttachmentUploadRequest, SignedAttachmentUrl } from "./types";
import { sanitizeAttachmentName } from "./textRendering";

export const MAX_ATTACHMENT_BYTES = 15 * 1024 * 1024;
export const MAX_ATTACHMENT_COUNT = 10;
export type AttachmentUploadApi = {
  createAttachmentUpload?: (request: AttachmentUploadRequest) => Promise<{
    upload_id: string;
    upload: SignedAttachmentUrl;
  }>;
};

export async function uploadAttachment(
  api: AttachmentUploadApi,
  file: File,
  signal: AbortSignal,
) {
  if (!api.createAttachmentUpload)
    throw new Error("Attachment uploads are unavailable");
  const digest = await crypto.subtle.digest(
    "SHA-256",
    await file.arrayBuffer(),
  );
  signal.throwIfAborted();
  const grant = await api.createAttachmentUpload({
    filename: sanitizeAttachmentName(file.name),
    content_type: file.type || "application/octet-stream",
    size_bytes: file.size,
    checksum_sha256: btoa(String.fromCharCode(...new Uint8Array(digest))),
  });
  signal.throwIfAborted();
  const response = await fetch(grant.upload.url, {
    method: "PUT",
    body: file,
    headers: grant.upload.headers,
    credentials: "omit",
    redirect: "error",
    signal,
  });
  if (!response.ok)
    throw new Error(
      `Upload failed (${response.status}). Attach the file again to retry.`,
    );
  return grant.upload_id;
}

export function saveAttachmentDownload(download: SignedAttachmentUrl) {
  // S3 supplies attachment Content-Disposition. Navigating directly avoids both
  // the WAF and copying the user's application token to another host.
  const anchor = document.createElement("a");
  anchor.href = download.url;
  anchor.rel = "noreferrer";
  anchor.referrerPolicy = "no-referrer";
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
}

export function displayFileSize(bytes: number) {
  return bytes >= 1024 * 1024
    ? `${(bytes / (1024 * 1024)).toFixed(1)} MB`
    : `${Math.max(1, Math.ceil(bytes / 1024))} KB`;
}
