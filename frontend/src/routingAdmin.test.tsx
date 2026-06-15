/* eslint-disable max-lines-per-function */
import "@testing-library/jest-dom/vitest";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { RoutingAdmin, type RoutingAdminApi } from "./routingAdmin";
import type { DomainConfig, UpdateDomainRequest } from "./types";

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
  const calls: string[] = [];
  const api: RoutingAdminApi = {
    listDomains: async () => [domain],
    createDomain: async (request) => {
      calls.push(`create:${JSON.stringify(request)}`);
      domain = {
        domain_name: request.domain_name.toLowerCase(),
        routing_policy: request.routing_policy ?? "allowlist",
        active: request.active ?? true,
        raw_retention_days: request.raw_retention_days ?? null,
        addresses: [],
      };
      return domain;
    },
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
  };
  return { api, calls };
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
      await screen.findByLabelText("Accepted recipient policy for ahara.io"),
      "catchall",
    );

    expect(calls).toContain('update:{"routing_policy":"catchall"}');
    expect(
      screen.getByLabelText("Accepted recipient policy for ahara.io"),
    ).toHaveValue("catchall");
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

  it("renders an empty state when no domains are configured", async () => {
    const { api } = apiWithDomain();
    api.listDomains = async () => [];

    render(<RoutingAdmin apiClient={api} />);

    expect(
      await screen.findByText("No mail domains configured"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Add mail domain" }),
    ).toBeInTheDocument();
  });

  it("creates mail domains from the empty state", async () => {
    const user = userEvent.setup();
    const { api, calls } = apiWithDomain();
    api.listDomains = async () => [];

    render(<RoutingAdmin apiClient={api} />);
    await user.type(await screen.findByLabelText("Mail domain"), "Ahara.IO");
    await user.selectOptions(
      screen.getByLabelText("Accepted recipient policy"),
      "catchall",
    );
    await user.click(screen.getByRole("button", { name: "Add mail domain" }));

    expect(calls).toContain(
      'create:{"domain_name":"Ahara.IO","routing_policy":"catchall"}',
    );
    expect(await screen.findByText("ahara.io")).toBeInTheDocument();
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
