/* eslint-disable max-lines-per-function */
import "@testing-library/jest-dom/vitest";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { RoutingAdmin, type RoutingAdminApi } from "./routingAdmin";
import type {
  DomainConfig,
  ForwardingRule,
  UpdateDomainRequest,
} from "./types";

const baseDomain: DomainConfig = {
  domain_name: "ahara.io",
  routing_policy: "allowlist",
  active: true,
  raw_retention_days: 365,
  addresses: [
    { local_part: "chris", active: true, raw_retention_days: null },
    { local_part: "contact", active: false, raw_retention_days: 30 },
  ],
};

function apiWithDomain(initial: DomainConfig = baseDomain) {
  let domain = structuredClone(initial);
  let forwardingRules: ForwardingRule[] = [];
  const calls: string[] = [];
  const api: RoutingAdminApi = {
    listDomains: async () => [domain],
    updateDomain: async (_domainName: string, request: UpdateDomainRequest) => {
      calls.push(`update:${JSON.stringify(request)}`);
      domain = { ...domain, ...request };
      return domain;
    },
    addAddress: async (_domainName: string, localPart: string) => {
      calls.push(`add:${localPart}`);
      domain = {
        ...domain,
        addresses: [
          ...domain.addresses,
          { local_part: localPart, active: true, raw_retention_days: null },
        ],
      };
      return { local_part: localPart, active: true, raw_retention_days: null };
    },
    updateAddress: async (_domainName, localPart, request) => {
      calls.push(`update-address:${localPart}:${JSON.stringify(request)}`);
      const updated = {
        local_part: localPart,
        active: request.active ?? true,
        raw_retention_days: request.raw_retention_days ?? null,
      };
      domain = {
        ...domain,
        addresses: domain.addresses.map((address) =>
          address.local_part === localPart ? updated : address,
        ),
      };
      return updated;
    },
    deactivateAddress: async (_domainName: string, localPart: string) => {
      calls.push(`deactivate:${localPart}`);
      domain = {
        ...domain,
        addresses: domain.addresses.map((address) =>
          address.local_part === localPart
            ? { ...address, active: false }
            : address,
        ),
      };
      return { local_part: localPart, active: false, raw_retention_days: null };
    },
    listForwardingRules: async () => forwardingRules,
    upsertForwardingRule: async (request) => {
      calls.push(`forward:${request.local_part}:${request.target_address}`);
      const existing = forwardingRules.find(
        (rule) =>
          rule.domain_name === request.domain_name &&
          rule.local_part === request.local_part &&
          rule.target_address_normalized ===
            request.target_address.toLowerCase(),
      );
      const rule = forwardingRuleFromRequest(
        existing?.id ?? `rule-${forwardingRules.length + 1}`,
        request,
      );
      forwardingRules = existing
        ? forwardingRules.map((item) => (item.id === existing.id ? rule : item))
        : [...forwardingRules, rule];
      return rule;
    },
    deactivateForwardingRule: async (ruleId) => {
      calls.push(`deactivate-forward:${ruleId}`);
      const rule = forwardingRules.find((item) => item.id === ruleId);
      if (!rule) {
        throw new Error("not found");
      }
      const updated = { ...rule, active: false };
      forwardingRules = forwardingRules.map((item) =>
        item.id === ruleId ? updated : item,
      );
      return updated;
    },
  };
  return { api, calls };
}

function forwardingRuleFromRequest(
  id: string,
  request: Parameters<RoutingAdminApi["upsertForwardingRule"]>[0],
): ForwardingRule {
  return {
    id,
    rule_kind: request.local_part ? "address" : "domain",
    domain_name: request.domain_name,
    local_part: request.local_part ?? null,
    address_id: request.local_part
      ? `${request.domain_name}:${request.local_part}`
      : null,
    target_address: request.target_address,
    target_address_normalized: request.target_address.toLowerCase(),
    sender_address_normalized: request.sender_address?.toLowerCase() ?? null,
    plus_tag: request.plus_tag?.toLowerCase() ?? null,
    require_auth_pass: request.require_auth_pass ?? true,
    active: true,
    created_at: null,
    updated_at: null,
  };
}

afterEach(() => cleanup());

describe("RoutingAdmin", () => {
  it("renders configured domains and accepted addresses", async () => {
    const { api } = apiWithDomain();

    render(<RoutingAdmin apiClient={api} />);

    expect(await screen.findByText("ahara.io")).toBeInTheDocument();
    expect(screen.getAllByText("chris").length).toBeGreaterThan(0);
    expect(screen.getAllByText("contact").length).toBeGreaterThan(0);
  });

  it("updates allowlist and catchall policy", async () => {
    const user = userEvent.setup();
    const { api, calls } = apiWithDomain();

    render(<RoutingAdmin apiClient={api} />);
    await user.selectOptions(
      await screen.findByLabelText("Routing policy for ahara.io"),
      "catchall",
    );

    expect(calls).toContain('update:{"routing_policy":"catchall"}');
    expect(screen.getByLabelText("Routing policy for ahara.io")).toHaveValue(
      "catchall",
    );
  });

  it("updates domain active state", async () => {
    const user = userEvent.setup();
    const { api, calls } = apiWithDomain();

    render(<RoutingAdmin apiClient={api} />);
    await user.click(await screen.findByLabelText("Deactivate domain"));

    expect(calls).toContain('update:{"active":false}');
  });

  it("updates domain and address raw retention overrides", async () => {
    const user = userEvent.setup();
    const { api, calls } = apiWithDomain();

    render(<RoutingAdmin apiClient={api} />);
    const domainRetention = await screen.findByLabelText("Raw retention days");
    await user.clear(domainRetention);
    await user.type(domainRetention, "45");
    await user.click(screen.getByRole("button", { name: "Save retention" }));

    const addressRetention = screen.getByLabelText(
      "Raw retention days for chris@ahara.io",
    );
    await user.type(addressRetention, "90");
    await user.click(
      within(addressRetention.closest("li") ?? document.body).getByRole(
        "button",
        { name: "Save" },
      ),
    );

    expect(calls).toContain('update:{"raw_retention_days":45}');
    expect(calls).toContain('update-address:chris:{"raw_retention_days":90}');
  });

  it("adds accepted local parts", async () => {
    const user = userEvent.setup();
    const { api, calls } = apiWithDomain();

    render(<RoutingAdmin apiClient={api} />);
    await user.type(
      await screen.findByLabelText("Add address for ahara.io"),
      "support",
    );
    await user.click(screen.getByRole("button", { name: "Add address" }));

    expect(calls).toContain("add:support");
    expect((await screen.findAllByText("support")).length).toBeGreaterThan(0);
  });

  it("deactivates accepted local parts", async () => {
    const user = userEvent.setup();
    const { api, calls } = apiWithDomain();

    render(<RoutingAdmin apiClient={api} />);
    await user.click(await screen.findByLabelText("Deactivate chris"));

    expect(calls).toContain("deactivate:chris");
  });

  it("adds and deactivates address-scoped forwarding rules", async () => {
    const user = userEvent.setup();
    const { api, calls } = apiWithDomain();

    render(<RoutingAdmin apiClient={api} />);
    await screen.findByText("Forwarding rules");
    await user.selectOptions(screen.getByLabelText("Source address"), "chris");
    await user.type(screen.getByLabelText("Forward to"), "target@example.com");
    await user.click(screen.getByRole("button", { name: "Add forwarding" }));

    expect(calls).toContain("forward:chris:target@example.com");
    expect(await screen.findByText("target@example.com")).toBeInTheDocument();

    await user.click(
      screen.getByLabelText("Deactivate forwarding rule target@example.com"),
    );

    expect(calls).toContain("deactivate-forward:rule-1");
  });

  it("renders authenticated API errors", async () => {
    const { api } = apiWithDomain();
    api.listDomains = async () => {
      throw new Error("unauthorized");
    };

    render(<RoutingAdmin apiClient={api} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("unauthorized");
  });

  it("does not expose MX or receipt-rule controls", async () => {
    const { api } = apiWithDomain();

    render(<RoutingAdmin apiClient={api} />);
    await screen.findByText("ahara.io");

    expect(
      screen.queryByText(/MX|mail exchange|receipt rule/i),
    ).not.toBeInTheDocument();
  });
});
