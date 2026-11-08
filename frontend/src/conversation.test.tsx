/* eslint-disable max-lines-per-function */
import "@testing-library/jest-dom/vitest";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  grantTestUpload,
  installUploadTransport,
} from "./mailUploads.testSupport";
import {
  act,
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ThreadDetail } from "./conversation";
import { MailboxList, MailboxView, type MailboxApi } from "./mailbox";
import {
  messageDetail,
  unreadMessage,
  sentMessage,
  sentMessageDetail,
} from "./mailbox.testSupport";
import type {
  MailboxMessageDetail,
  MailboxMessageSummary,
  MailboxThreadDetail,
} from "./types";

beforeEach(installUploadTransport);
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

const sentReply: MailboxMessageDetail = {
  ...messageDetail,
  id: "sent-reply",
  direction: "outbound",
  status: "sent",
  received_at: null,
  sent_at: "2026-01-01T01:00:00Z",
  created_at: "2026-01-01T00:59:00Z",
  from_address: "contact@ahara.io",
  from_display_name: "Chris",
  body_text: "My sent reply appears in this conversation.",
  attachments: [],
};

function thread(messages: MailboxMessageDetail[]): MailboxThreadDetail {
  return {
    thread_id: "thread-1",
    normalized_subject: "invoice",
    message_count: messages.length,
    last_activity_at: null,
    messages,
  };
}

function renderMailbox(apiClient: MailboxApi) {
  return render(
    <MailboxView
      apiClient={{ createAttachmentUpload: grantTestUpload, ...apiClient }}
    />,
  );
}

function renderList(messages: MailboxMessageSummary[]) {
  return render(<MailboxList messages={messages} />);
}

describe("Conversations", () => {
  it("shows sent replies, expands earlier messages, and discloses scan details on demand", async () => {
    const user = userEvent.setup();
    const { container } = render(
      <ThreadDetail thread={thread([messageDetail, sentReply])} />,
    );
    expect(
      screen.getByText("My sent reply appears in this conversation."),
    ).toBeVisible();
    expect(screen.getByText("sent")).toBeVisible();
    expect(container.querySelectorAll(".conversation-message")).toHaveLength(2);
    await user.click(
      screen.getByRole("button", {
        name: "Expand message from sender@example.test",
      }),
    );
    expect(screen.getByText(/Authenticated · Scans passed/)).toBeVisible();
    expect(screen.getByText("SPF")).not.toBeVisible();
    await user.click(screen.getByText(/Authenticated · Scans passed/));
    expect(screen.getByText("SPF")).toBeVisible();
    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector(".message-body-text a")).toBeNull();
  });

  it("keeps security warnings visible on collapsed received messages", () => {
    render(
      <ThreadDetail
        thread={thread([{ ...messageDetail, auth_verdict: "fail" }, sentReply])}
      />,
    );
    expect(screen.getByText("Security warning")).toBeVisible();
  });

  it("groups received messages from the same thread and preserves unread state", () => {
    const messages = [
      { ...unreadMessage, id: "latest", is_read: true },
      unreadMessage,
    ];
    const { container } = renderList(messages);
    expect(container.querySelectorAll(".mailbox-row")).toHaveLength(1);
    expect(screen.getByLabelText("2 received messages")).toBeVisible();
    expect(screen.getByLabelText("Unread message")).toBeVisible();
  });

  it("refreshes the conversation with the queued reply after sending", async () => {
    const user = userEvent.setup();
    let messages = [messageDetail];
    renderMailbox({
      fetchMailboxMessages: async () => [unreadMessage],
      fetchThreadDetail: async () => thread(messages),
      replyToMessage: async (_id, request) => {
        messages = [
          ...messages,
          { ...sentReply, status: "queued", body_text: request.body_text },
        ];
        return {
          message_id: "sent-reply",
          work_id: "work-1",
          rfc_message_id: "<reply@ahara.io>",
          status: "queued",
          recipients: [],
        };
      },
    });
    await user.click(
      await screen.findByRole("button", { name: /sender@example.test/ }),
    );
    await user.click(screen.getByRole("button", { name: "Reply" }));
    await user.type(
      screen.getByRole("textbox", { name: "Reply" }),
      "New reply in the conversation",
    );
    await user.click(screen.getByRole("button", { name: "Send reply" }));
    expect(await screen.findByText("queued")).toBeVisible();
    expect(screen.getByText("New reply in the conversation")).toBeVisible();
  });

  it("prevents duplicate submissions and retains the draft and file when sending fails", async () => {
    const user = userEvent.setup();
    let rejectReply: (error: Error) => void = () => {};
    let calls = 0;
    renderMailbox({
      fetchMailboxMessages: async () => [unreadMessage],
      fetchThreadDetail: async () => thread([messageDetail]),
      replyToMessage: async () => {
        calls += 1;
        return new Promise((_resolve, reject) => {
          rejectReply = reject;
        });
      },
    });
    await user.click(
      await screen.findByRole("button", { name: /sender@example.test/ }),
    );
    await user.click(screen.getByRole("button", { name: "Reply" }));
    await user.type(
      screen.getByRole("textbox", { name: "Reply" }),
      "Keep this draft",
    );
    await user.upload(
      screen.getByLabelText("Reply attachments"),
      new File(["notes"], "notes.txt", { type: "text/plain" }),
    );
    expect(await screen.findByText("notes.txt")).toBeVisible();
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Send reply" })).toBeEnabled(),
    );
    await user.click(screen.getByRole("button", { name: "Send reply" }));
    expect(screen.getByRole("button", { name: "Sending…" })).toBeDisabled();
    await act(async () => {
      rejectReply(new TypeError("Failed to fetch"));
    });
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent("Failed to fetch"),
    );
    expect(screen.getByRole("textbox", { name: "Reply" })).toHaveValue(
      "Keep this draft",
    );
    expect(screen.getByText("notes.txt")).toBeVisible();
    expect(calls).toBe(1);
    expect(screen.getByRole("button", { name: "Send reply" })).toBeEnabled();
  });

  it("opens the full conversation from Sent and can reply to its received message", async () => {
    const user = userEvent.setup();
    let messages = [messageDetail, sentReply];
    let replySource = "";
    renderMailbox({
      fetchMailboxMessages: async () => [unreadMessage],
      fetchThreadDetail: async () => thread(messages),
      listOutboundMessages: async () => [sentMessage],
      fetchOutboundMessage: async () => sentMessageDetail,
      replyToMessage: async (id, request) => {
        replySource = id;
        messages = [
          ...messages,
          {
            ...sentReply,
            id: "next-reply",
            status: "queued",
            body_text: request.body_text,
          },
        ];
        return {
          message_id: "next-reply",
          work_id: "work-2",
          rfc_message_id: "<next@ahara.io>",
          status: "queued",
          recipients: [],
        };
      },
    });
    await user.click(
      await screen.findByRole("button", { name: "Sent", exact: true }),
    );
    await user.click(
      await screen.findByRole("button", { name: /Queued body/ }),
    );
    const conversation = await screen.findByLabelText("Thread detail");
    expect(within(conversation).getByText(sentReply.body_text)).toBeVisible();
    await user.click(
      within(conversation).getByRole("button", { name: "Reply", exact: true }),
    );
    await user.type(
      screen.getByRole("textbox", { name: "Reply", exact: true }),
      "Reply from the Sent conversation",
    );
    await user.click(screen.getByRole("button", { name: "Send reply" }));
    await within(conversation).findByText("queued");
    expect(
      within(conversation).getByText("Reply from the Sent conversation", {
        selector: "pre",
      }),
    ).toBeVisible();
    expect(replySource).toBe(messageDetail.id);
  });

  it("groups sent replies and keeps legacy sent attachments downloadable", async () => {
    const user = userEvent.setup();
    const attachment = {
      position: 0,
      filename: "notes.txt",
      display_filename: "notes.txt",
      content_type: "text/plain",
      size_bytes: 5,
    };
    const download = vi.fn(async () => ({
      url: "https://private-mail.s3.amazonaws.com/notes",
      headers: {},
      expires_in_seconds: 900,
    }));
    const click = vi
      .spyOn(HTMLAnchorElement.prototype, "click")
      .mockImplementation(() => {});
    renderMailbox({
      fetchMailboxMessages: async () => [],
      listOutboundMessages: async () => [
        sentMessage,
        { ...sentMessage, id: "earlier" },
      ],
      fetchOutboundMessage: async () => ({
        ...sentMessageDetail,
        thread_id: null,
        attachments: [attachment],
      }),
      downloadOutboundAttachment: download,
    });
    await user.click(
      await screen.findByRole("button", { name: "Sent", exact: true }),
    );
    expect(await screen.findByLabelText("2 sent messages")).toBeVisible();
    expect(screen.getAllByRole("button", { name: /Queued body/ })).toHaveLength(
      1,
    );
    await user.click(screen.getByRole("button", { name: /Queued body/ }));
    await user.click(
      await screen.findByRole("button", { name: "Download notes.txt" }),
    );
    expect(download).toHaveBeenCalledWith(sentMessageDetail.id, 0);
    expect(click).toHaveBeenCalledOnce();
    click.mockRestore();
  });

  it("does not turn a successful send into a failure if refreshing the thread fails", async () => {
    const user = userEvent.setup();
    let reads = 0;
    let sends = 0;
    renderMailbox({
      fetchMailboxMessages: async () => [unreadMessage],
      fetchThreadDetail: async () => {
        if (reads++ > 0) throw new Error("refresh failed");
        return thread([messageDetail]);
      },
      replyToMessage: async () => {
        sends += 1;
        return {
          message_id: "sent-reply",
          work_id: "work-1",
          rfc_message_id: "<reply@ahara.io>",
          status: "queued",
          recipients: [],
        };
      },
    });
    await user.click(
      await screen.findByRole("button", { name: /sender@example.test/ }),
    );
    await user.click(
      screen.getByRole("button", { name: "Reply", exact: true }),
    );
    await user.type(
      screen.getByRole("textbox", { name: "Reply", exact: true }),
      "Queue this once",
    );
    await user.click(screen.getByRole("button", { name: "Send reply" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Your reply was queued",
    );
    expect(
      screen.queryByRole("textbox", { name: "Reply", exact: true }),
    ).not.toBeInTheDocument();
    expect(sends).toBe(1);
  });
});
