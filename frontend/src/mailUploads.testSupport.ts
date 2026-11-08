import { vi } from "vitest";

export function installUploadTransport() {
  vi.stubGlobal("crypto", {
    subtle: { digest: async () => new ArrayBuffer(32) },
  });
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => new Response(null, { status: 200 })),
  );
}

export async function grantTestUpload() {
  return {
    upload_id: "upload-1",
    upload: {
      url: "https://private-mail.s3.amazonaws.com/upload?signature=test",
      headers: { "content-type": "application/octet-stream" },
      expires_in_seconds: 900,
    },
  };
}
