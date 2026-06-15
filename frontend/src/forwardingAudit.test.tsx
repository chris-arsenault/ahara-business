import "@testing-library/jest-dom/vitest";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ForwardingView } from "./forwardingAudit";
import type { ForwardingApi } from "./forwardingAuditTypes";
import type {
  DomainConfig,
  ForwardingMessageStatus,
  ForwardingRule,
  ForwardingRuleStatus,
} from "./types";

const domain: DomainConfig = {
  domain_name: "ahara.io",
  routing_policy: "allowlist",
  active: true,
  raw_retention_days: 365,
  addresses: [
    { local_part: "chris", active: true, raw_retention_days: null },
    { local_part: "contact", active: true, raw_retention_days: null },
  ],
};

afterEach(() => cleanup());

describe("ForwardingView", () => {
  it("renders empty forwarding states", async () => {
    render(<ForwardingView apiClient={api().apiClient} />);

    expect(
      await screen.findByText("No forwarding rules yet"),
    ).toBeInTheDocument();
    expect(screen.getByText("No forwarding activity yet")).toBeInTheDocument();
    expect(screen.getByText("No matched messages yet")).toBeInTheDocument();
  });

  it("adds and deactivates forwarding rules", async () => {
    const user = userEvent.setup();
    const { apiClient, calls } = api();

    render(<ForwardingView apiClient={apiClient} />);
    await user.selectOptions(
      await screen.findByLabelText("Source address"),
      "chris",
    );
    await user.type(screen.getByLabelText("Forward to"), "target@example.com");
    await user.click(screen.getByRole("button", { name: "Add forwarding" }));

    expect(calls).toContain("forward:chris:target@example.com");
    expect(await screen.findByText("target@example.com")).toBeInTheDocument();

    await user.click(
      screen.getByLabelText("Deactivate forwarding rule target@example.com"),
    );

    expect(calls).toContain("deactivate-forward:rule-1");
  });

  it("renders rule and message forwarding status", async () => {
    render(
      <ForwardingView
        apiClient={
          api({
            messageStatuses: [messageStatus()],
            ruleStatuses: [ruleStatus()],
            rules: [rule()],
          }).apiClient
        }
      />,
    );

    expect(
      (await screen.findAllByText("contact@ahara.io")).length,
    ).toBeGreaterThan(0);
    expect(screen.getAllByText("target@example.com").length).toBeGreaterThan(0);
    expect(screen.getByText("Inbound invoice")).toBeInTheDocument();
    expect(screen.getByText("sender@example.com")).toBeInTheDocument();
  });
});

function api(
  initial: Partial<{
    messageStatuses: ForwardingMessageStatus[];
    ruleStatuses: ForwardingRuleStatus[];
    rules: ForwardingRule[];
  }> = {},
) {
  let rules = structuredClone(initial.rules ?? []);
  const calls: string[] = [];
  const apiClient: ForwardingApi = {
    listDomains: async () => [domain],
    listForwardingRules: async () => rules,
    upsertForwardingRule: async (request) => {
      calls.push(`forward:${request.local_part}:${request.target_address}`);
      const nextRule = ruleFromRequest(`rule-${rules.length + 1}`, request);
      rules = [...rules, nextRule];
      return nextRule;
    },
    deactivateForwardingRule: async (ruleId) => {
      calls.push(`deactivate-forward:${ruleId}`);
      const existing = rules.find((item) => item.id === ruleId);
      if (!existing) {
        throw new Error("not found");
      }
      const updated = { ...existing, active: false };
      rules = rules.map((item) => (item.id === ruleId ? updated : item));
      return updated;
    },
    listForwardingRuleStatuses: async () => initial.ruleStatuses ?? [],
    listForwardingMessageStatuses: async () => initial.messageStatuses ?? [],
  };
  return { apiClient, calls };
}

function rule(): ForwardingRule {
  return {
    id: "rule-1",
    rule_kind: "address",
    domain_name: "ahara.io",
    local_part: "contact",
    address_id: "ahara.io:contact",
    target_address: "target@example.com",
    target_address_normalized: "target@example.com",
    sender_address_normalized: null,
    plus_tag: null,
    require_auth_pass: true,
    active: true,
    created_at: null,
    updated_at: null,
  };
}

function ruleFromRequest(
  id: string,
  request: Parameters<ForwardingApi["upsertForwardingRule"]>[0],
): ForwardingRule {
  return {
    ...rule(),
    id,
    rule_kind: request.local_part ? "address" : "domain",
    local_part: request.local_part ?? null,
    target_address: request.target_address,
    target_address_normalized: request.target_address.toLowerCase(),
  };
}

function ruleStatus(): ForwardingRuleStatus {
  return {
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
  };
}

function messageStatus(): ForwardingMessageStatus {
  return {
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
  };
}
