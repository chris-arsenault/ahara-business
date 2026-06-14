import "@testing-library/jest-dom/vitest";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { ForwardingAuditView } from "./forwardingAudit";
import type { ForwardingAuditApi } from "./forwardingAuditTypes";

afterEach(() => cleanup());

describe("ForwardingAuditView", () => {
  it("renders rule and message forwarding status", async () => {
    render(<ForwardingAuditView apiClient={api()} />);

    expect(await screen.findByText("contact@ahara.io")).toBeInTheDocument();
    expect(screen.getByText("target@example.com")).toBeInTheDocument();
    expect(screen.getByText("Inbound invoice")).toBeInTheDocument();
    expect(screen.getByText("sender@example.com")).toBeInTheDocument();
  });
});

function api(): ForwardingAuditApi {
  return {
    listForwardingRuleStatuses: async () => [
      {
        rule_id: "rule-1",
        rule_kind: "address",
        domain_name: "ahara.io",
        local_part: "contact",
        target_address: "target@example.com",
        active: true,
        queued_count: 0,
        sending_count: 0,
        sent_count: 1,
        failed_count: 0,
        bounced_count: 0,
        complained_count: 0,
        last_attempt_at: "now",
        last_error: null,
      },
    ],
    listForwardingMessageStatuses: async () => [
      {
        source_message_id: "message-1",
        thread_id: "thread-1",
        subject: "Inbound invoice",
        from_address: "sender@example.com",
        received_at: "now",
        matching_rule_count: 1,
        queued_count: 0,
        sending_count: 0,
        sent_count: 1,
        failed_count: 0,
        bounced_count: 0,
        complained_count: 0,
        last_error: null,
      },
    ],
  };
}
