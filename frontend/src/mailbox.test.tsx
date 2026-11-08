/* eslint-disable max-lines-per-function */
import "@testing-library/jest-dom/vitest";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import {
  grantTestUpload,
  installUploadTransport,
} from "./mailUploads.testSupport";
import userEvent from "@testing-library/user-event";
import { MailboxList, MailboxView, type MailboxApi } from "./mailbox";
import type { MailboxMessageSummary } from "./types";

import {
  unreadMessage,
  readMessage,
  messageDetail,
  sentMessage,
  sentMessageDetail,
} from "./mailbox.testSupport";

beforeEach(installUploadTransport);
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

function renderMailboxList(messages: MailboxMessageSummary[]) {
  return render(<MailboxList messages={messages} />);
}

function renderMailboxView(apiClient: MailboxApi) {
  return render(
    <MailboxView
      apiClient={{ createAttachmentUpload: grantTestUpload, ...apiClient }}
    />,
  );
}

describe("MailboxList", () => {
  it("renders compact mailbox rows with unread state, dates, and attachments", () => {
    renderMailboxList([unreadMessage]);

    expect(screen.getAllByText("sender@example.test").length).toBeGreaterThan(
      0,
    );
    expect(screen.getAllByText("Sender Display").length).toBeGreaterThan(0);
    expect(screen.getByText("Invoice")).toBeInTheDocument();
    expect(screen.getByText("Plaintext invoice body")).toBeInTheDocument();
    expect(screen.getByLabelText("Unread message")).toBeInTheDocument();
    expect(screen.queryByText("pass")).not.toBeInTheDocument();
    expect(screen.getByText("Jan 1")).toBeInTheDocument();
    expect(screen.getByLabelText("Attachment count")).toHaveTextContent("2");
  });

  it("renders an empty accepted-mailbox state", () => {
    renderMailboxList([]);

    expect(screen.getByText("No accepted messages")).toBeInTheDocument();
  });
});

describe("MailboxView", () => {
  it("loads mailbox messages from the API", async () => {
    renderMailboxView({ fetchMailboxMessages: async () => [unreadMessage] });

    expect(await screen.findByText("Invoice")).toBeInTheDocument();
  });

  it("renders API load errors", async () => {
    renderMailboxView({
      fetchMailboxMessages: async () => {
        throw new Error("load failed");
      },
    });

    expect(await screen.findByRole("alert")).toHaveTextContent("load failed");
  });

  it("renders thread detail as inert plaintext with auth and attachment metadata", async () => {
    const user = userEvent.setup();
    const { container } = renderMailboxView({
      fetchMailboxMessages: async () => [unreadMessage],
      fetchThreadDetail: async () => ({
        thread_id: "thread-1",
        normalized_subject: "invoice",
        message_count: 1,
        last_activity_at: "2026-01-01 00:00:00+00",
        messages: [messageDetail],
      }),
    });

    await user.click(
      await screen.findByRole("button", { name: /sender@example.test/i }),
    );

    expect(screen.getAllByText("sender@example.test").length).toBeGreaterThan(
      0,
    );
    expect(screen.getAllByText("Sender Display").length).toBeGreaterThan(0);
    await user.click(screen.getByText(/Authenticated · Scans passed/));
    expect(screen.getByText("Auth verdict")).toBeVisible();
    expect(screen.getAllByText("pass").length).toBeGreaterThan(0);
    expect(
      screen.getByText(/<script>alert\(1\)<\/script>/),
    ).toBeInTheDocument();
    expect(screen.getByText(/javascript:alert\(1\)/)).toBeInTheDocument();
    expect(screen.getByText(/data:text\/html/)).toBeInTheDocument();
    expect(container.querySelector("script")).not.toBeInTheDocument();
    expect(
      container.querySelector(".message-body-text a"),
    ).not.toBeInTheDocument();
    expect(screen.getByText("invoice.pdf")).toBeInTheDocument();
    expect(screen.getByText(/application\/pdf/)).toBeInTheDocument();
  });

  it("marks messages read from the list", async () => {
    const user = userEvent.setup();
    let requested: { messageId: string; isRead: boolean } | undefined;
    renderMailboxView({
      fetchMailboxMessages: async () => [unreadMessage],
      updateMessageState: async (messageId, isRead) => {
        requested = { messageId, isRead };
        return { ...unreadMessage, is_read: isRead };
      },
    });

    await user.click(await screen.findByLabelText("Mark read"));

    expect(requested).toEqual({ messageId: "message-1", isRead: true });
    expect(await screen.findByLabelText("Mark unread")).toBeInTheDocument();
  });

  it("marks messages unread from the list", async () => {
    const user = userEvent.setup();
    renderMailboxView({
      fetchMailboxMessages: async () => [readMessage],
      updateMessageState: async (_messageId, isRead) => ({
        ...readMessage,
        is_read: isRead,
      }),
    });

    await user.click(await screen.findByLabelText("Mark unread"));

    expect(await screen.findByLabelText("Unread message")).toBeInTheDocument();
  });

  it("links and unlinks contacts through explicit selection", async () => {
    const user = userEvent.setup();
    const contactCalls: Array<string | null> = [];
    renderMailboxView({
      fetchMailboxMessages: async () => [unreadMessage],
      fetchThreadDetail: async () => ({
        thread_id: "thread-1",
        normalized_subject: "invoice",
        message_count: 1,
        last_activity_at: "2026-01-01 00:00:00+00",
        messages: [messageDetail],
      }),
      listContacts: async () => [
        {
          id: "contact-1",
          display_name: "Chris",
          primary_address: "chris@example.test",
          primary_address_normalized: "chris@example.test",
          notes: "",
        },
      ],
      linkMessageContact: async (_messageId, contactId) => {
        contactCalls.push(contactId);
        return { ...unreadMessage, contact_id: contactId };
      },
    });

    await user.click(
      await screen.findByRole("button", { name: /sender@example.test/i }),
    );
    const select = await screen.findByLabelText("Contact association");
    await user.selectOptions(select, "contact-1");
    await user.selectOptions(select, "");

    expect(contactCalls).toEqual(["contact-1", null]);
  });

  it("shows API error state for failed message actions", async () => {
    const user = userEvent.setup();
    renderMailboxView({
      fetchMailboxMessages: async () => [unreadMessage],
      updateMessageState: async () => {
        throw new Error("update failed");
      },
    });

    await user.click(await screen.findByLabelText("Mark read"));

    expect(await screen.findByRole("alert")).toHaveTextContent("update failed");
  });

  it("submits compose messages with attachments", async () => {
    const user = userEvent.setup();
    let composed:
      | {
          from_address: string;
          to: string[];
          subject: string;
          body_text: string;
          attachments: Array<{
            upload_id: string;
          }>;
        }
      | undefined;
    renderMailboxView({
      fetchMailboxMessages: async () => [],
      composeMessage: async (request) => {
        composed = request;
        return {
          message_id: "outbound-1",
          work_id: "work-1",
          rfc_message_id: "<outbound-1@ahara.io>",
          status: "queued",
          recipients: [],
        };
      },
    });

    await user.click(await screen.findByRole("button", { name: "Compose" }));
    await user.clear(screen.getByLabelText("From"));
    await user.type(screen.getByLabelText("From"), "contact@ahara.io");
    await user.type(screen.getByLabelText("To"), "person@example.com");
    await user.type(screen.getByLabelText("Subject"), "Plain note");
    await user.type(screen.getByLabelText("Body"), "hello <b>world</b>");
    await user.upload(
      screen.getByLabelText("Attachments"),
      new File(["hi"], "invoice.pdf", { type: "application/pdf" }),
    );
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Send" })).toBeEnabled(),
    );
    await user.click(screen.getByRole("button", { name: "Send" }));

    expect(composed).toEqual({
      from_address: "contact@ahara.io",
      to: ["person@example.com"],
      subject: "Plain note",
      body_text: "hello <b>world</b>",
      cc: [],
      bcc: [],
      attachments: [
        {
          upload_id: "upload-1",
        },
      ],
    });
    expect(screen.getByRole("dialog", { hidden: true })).not.toBeVisible();
  });

  it("shows sent mail and opens outbound message details", async () => {
    const user = userEvent.setup();
    let fetchedOutboundId = "";
    renderMailboxView({
      fetchMailboxMessages: async () => [],
      listOutboundMessages: async () => [sentMessage],
      fetchOutboundMessage: async (messageId) => {
        fetchedOutboundId = messageId;
        return sentMessageDetail;
      },
    });

    await user.click(await screen.findByRole("button", { name: "Sent" }));
    expect(await screen.findByText("Plain note")).toBeInTheDocument();
    expect(screen.getByText("person@example.com")).toBeInTheDocument();
    expect(screen.getByText("queued")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Queued body/i }));

    expect(fetchedOutboundId).toBe("outbound-1");
    expect(await screen.findByLabelText("Thread detail")).toBeInTheDocument();
    expect(screen.getAllByText("contact@ahara.io").length).toBeGreaterThan(0);
    expect(
      screen.getByText(/<script>alert\(1\)<\/script>/),
    ).toBeInTheDocument();
  });

  it("submits replies with attachments from message detail", async () => {
    const user = userEvent.setup();
    let reply:
      | {
          messageId: string;
          from_address: string;
          body_text: string;
          attachments: Array<{
            upload_id: string;
          }>;
        }
      | undefined;
    renderMailboxView({
      fetchMailboxMessages: async () => [unreadMessage],
      fetchThreadDetail: async () => ({
        thread_id: "thread-1",
        normalized_subject: "invoice",
        message_count: 1,
        last_activity_at: "2026-01-01 00:00:00+00",
        messages: [messageDetail],
      }),
      replyToMessage: async (messageId, request) => {
        reply = {
          messageId,
          from_address: request.from_address,
          body_text: request.body_text,
          attachments: request.attachments ?? [],
        };
        return {
          message_id: "outbound-1",
          work_id: "work-1",
          rfc_message_id: "<outbound-1@ahara.io>",
          status: "queued",
          recipients: [],
        };
      },
    });

    await user.click(
      await screen.findByRole("button", { name: /sender@example.test/i }),
    );
    await user.click(await screen.findByRole("button", { name: "Reply" }));
    await user.type(
      screen.getByRole("textbox", { name: "Reply" }),
      "reply <i>body</i>",
    );
    await user.upload(
      screen.getByLabelText("Reply attachments"),
      new File(["ok"], "reply.txt", { type: "text/plain" }),
    );
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Send reply" })).toBeEnabled(),
    );
    await user.click(screen.getByRole("button", { name: "Send reply" }));

    expect(reply).toEqual({
      messageId: "message-1",
      from_address: "contact@ahara.io",
      body_text: "reply <i>body</i>",
      attachments: [
        {
          upload_id: "upload-1",
        },
      ],
    });
  });

  it("does not auto-associate contacts from sender display names", async () => {
    const user = userEvent.setup();
    const contactCalls: Array<string | null> = [];
    renderMailboxView({
      fetchMailboxMessages: async () => [unreadMessage],
      fetchThreadDetail: async () => ({
        thread_id: "thread-1",
        normalized_subject: "invoice",
        message_count: 1,
        last_activity_at: "2026-01-01 00:00:00+00",
        messages: [messageDetail],
      }),
      listContacts: async () => [
        {
          id: "contact-1",
          display_name: "Sender Display",
          primary_address: "sender@example.test",
          primary_address_normalized: "sender@example.test",
          notes: "",
        },
      ],
      linkMessageContact: async (_messageId, contactId) => {
        contactCalls.push(contactId);
        return { ...unreadMessage, contact_id: contactId };
      },
    });

    await user.click(
      await screen.findByRole("button", { name: /sender@example.test/i }),
    );
    const select = (await screen.findByLabelText(
      "Contact association",
    )) as HTMLSelectElement;

    expect(select.value).toBe("");
    expect(contactCalls).toEqual([]);
  });

  it("submits mailbox searches and renders accepted results", async () => {
    const user = userEvent.setup();
    let searched = "";
    renderMailboxView({
      fetchMailboxMessages: async () => [],
      searchMessages: async (query) => {
        searched = query;
        return [unreadMessage];
      },
    });

    await user.type(
      await screen.findByPlaceholderText("Search mailbox"),
      "invoice",
    );
    await user.click(screen.getByRole("button", { name: "Search" }));

    expect(searched).toBe("invoice");
    expect(await screen.findByText("Invoice")).toBeInTheDocument();
  });

  it("renders empty search results", async () => {
    const user = userEvent.setup();
    renderMailboxView({
      fetchMailboxMessages: async () => [unreadMessage],
      searchMessages: async () => [],
    });

    await user.type(
      await screen.findByPlaceholderText("Search mailbox"),
      "missing",
    );
    await user.click(screen.getByRole("button", { name: "Search" }));

    expect(await screen.findByText("No accepted messages")).toBeInTheDocument();
  });

  it("shows search API errors", async () => {
    const user = userEvent.setup();
    renderMailboxView({
      fetchMailboxMessages: async () => [unreadMessage],
      searchMessages: async () => {
        throw new Error("search failed");
      },
    });

    await user.type(
      await screen.findByPlaceholderText("Search mailbox"),
      "invoice",
    );
    await user.click(screen.getByRole("button", { name: "Search" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("search failed");
  });

  it("keeps dangerous search result snippets inert", async () => {
    const user = userEvent.setup();
    const scriptLike = "java" + "script:alert(1)";
    const { container } = renderMailboxView({
      fetchMailboxMessages: async () => [],
      searchMessages: async () => [
        {
          ...unreadMessage,
          snippet: `${scriptLike} data:text/html,<script>alert(1)</script>`,
          security_disposition: "accepted",
        },
      ],
    });

    await user.type(
      await screen.findByPlaceholderText("Search mailbox"),
      "script",
    );
    await user.click(screen.getByRole("button", { name: "Search" }));

    expect(
      await screen.findByText((content) => content.includes(scriptLike)),
    ).toBeInTheDocument();
    expect(screen.queryByText("Rejected invoice body")).not.toBeInTheDocument();
    expect(container.querySelector(".mailbox-list a")).not.toBeInTheDocument();
  });
});
