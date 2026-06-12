import type {
  MailboxAttachmentDownload,
  OutboundAttachmentInput,
} from "./types";
import { sanitizeAttachmentName } from "./textRendering";

export async function readOutboundAttachments(
  files: FileList | null,
): Promise<OutboundAttachmentInput[]> {
  if (!files || files.length === 0) {
    return [];
  }
  return Promise.all(Array.from(files).map(fileToAttachment));
}

export function saveAttachmentDownload(download: MailboxAttachmentDownload) {
  const bytes = base64ToBytes(download.content_base64);
  const blob = new Blob([bytes], {
    type: download.content_type || "application/octet-stream",
  });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = sanitizeAttachmentName(
    download.display_filename || download.filename,
  );
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(url);
}

async function fileToAttachment(file: File): Promise<OutboundAttachmentInput> {
  return {
    filename: sanitizeAttachmentName(file.name),
    content_type: file.type || "application/octet-stream",
    content_base64: arrayBufferToBase64(await file.arrayBuffer()),
  };
}

function arrayBufferToBase64(buffer: ArrayBuffer) {
  let binary = "";
  for (const byte of new Uint8Array(buffer)) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary);
}

function base64ToBytes(value: string) {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}
