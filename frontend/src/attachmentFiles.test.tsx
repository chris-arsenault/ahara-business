/* eslint-disable max-lines-per-function */
import "@testing-library/jest-dom/vitest";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { uploadAttachment, MAX_ATTACHMENT_BYTES } from "./attachmentFiles";
import {
  ComposerAttachments,
  useComposerAttachments,
} from "./composerAttachments";
import { ComposeMessage } from "./composeMessage";
import {
  grantTestUpload,
  installUploadTransport,
} from "./mailUploads.testSupport";

beforeEach(installUploadTransport);
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

const api = { createAttachmentUpload: grantTestUpload };
function Files() {
  const model = useComposerAttachments(api);
  return (
    <>
      <ComposerAttachments model={model} disabled={false} />
      <button disabled={model.busy}>Send</button>
      <output>{JSON.stringify(model.references)}</output>
    </>
  );
}

describe("Direct mail attachment uploads", () => {
  it("PUTs raw bytes with signed headers and no application credentials", async () => {
    const create = vi.fn(grantTestUpload);
    const file = new File(["notes"], "notes.txt", { type: "text/plain" });
    await expect(
      uploadAttachment(
        { createAttachmentUpload: create },
        file,
        new AbortController().signal,
      ),
    ).resolves.toBe("upload-1");
    expect(create).toHaveBeenCalledWith({
      filename: "notes.txt",
      content_type: "text/plain",
      size_bytes: 5,
      checksum_sha256: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
    });
    expect(fetch).toHaveBeenCalledWith(
      expect.stringContaining("s3.amazonaws.com"),
      expect.objectContaining({
        method: "PUT",
        body: file,
        credentials: "omit",
        redirect: "error",
        headers: { "content-type": "application/octet-stream" },
      }),
    );
    expect(create.mock.calls[0]?.[0]).not.toHaveProperty("content_base64");
  });

  it("blocks Send while uploading, exposes failure and allows file removal", async () => {
    const user = userEvent.setup();
    let fail: (error: Error) => void = () => {};
    vi.stubGlobal(
      "fetch",
      vi.fn(
        () =>
          new Promise((_resolve, reject) => {
            fail = reject;
          }),
      ),
    );
    render(<Files />);
    await user.upload(
      screen.getByLabelText("Attachments"),
      new File(["notes"], "notes.txt"),
    );
    await waitFor(() => expect(fetch).toHaveBeenCalled());
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
    fail(new TypeError("Failed to fetch"));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Failed to fetch",
    );
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Remove notes.txt" }));
    expect(screen.queryByText("notes.txt")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Send" })).toBeEnabled();
  });

  it("rejects a combined selection above 15 MiB before granting uploads", async () => {
    const user = userEvent.setup();
    render(<Files />);
    await user.upload(screen.getByLabelText("Attachments"), [
      new File([new Uint8Array(MAX_ATTACHMENT_BYTES)], "large.bin"),
      new File(["x"], "extra.txt"),
    ]);
    expect(await screen.findByRole("alert")).toHaveTextContent("15 MB");
    expect(fetch).not.toHaveBeenCalled();
  });

  it("retains compose text when closed and reopened", async () => {
    const user = userEvent.setup();
    const props = {
      apiClient: { fetchMailboxMessages: async () => [] },
      onClose: vi.fn(),
      onQueued: vi.fn(),
    };
    const view = render(<ComposeMessage {...props} open />);
    await user.type(screen.getByLabelText("To"), "person@example.test");
    await user.type(screen.getByLabelText("Body"), "Draft remains here");
    view.rerender(<ComposeMessage {...props} open={false} />);
    view.rerender(<ComposeMessage {...props} open />);
    expect(screen.getByLabelText("Body")).toHaveValue("Draft remains here");
    expect(screen.getByLabelText("To")).toHaveValue("person@example.test");
  });
});
