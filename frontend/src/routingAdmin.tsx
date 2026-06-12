/* eslint-disable complexity, max-lines-per-function, react-hooks/exhaustive-deps, react-hooks/set-state-in-effect */
import { useEffect, useState } from "react";
import { Plus, Power, PowerOff, ShieldCheck, Trash2 } from "lucide-react";
import type { ApiClient } from "./api";
import type { DomainConfig, ForwardingRule, RoutingPolicy } from "./types";

export type RoutingAdminApi = Pick<
  ApiClient,
  | "listDomains"
  | "updateDomain"
  | "addAddress"
  | "deactivateAddress"
  | "listForwardingRules"
  | "upsertForwardingRule"
  | "deactivateForwardingRule"
>;

type RoutingState =
  | { status: "loading" }
  | {
      status: "ready";
      domains: DomainConfig[];
      forwardingRules: ForwardingRule[];
    }
  | { status: "error"; message: string };

export function RoutingAdmin({ apiClient }: { apiClient: RoutingAdminApi }) {
  const [state, setState] = useState<RoutingState>({ status: "loading" });
  const [draftLocalParts, setDraftLocalParts] = useState<
    Record<string, string>
  >({});
  const [draftForwarding, setDraftForwarding] = useState<
    Record<string, { local_part: string; target_address: string }>
  >({});
  const [actionError, setActionError] = useState<string>();

  async function loadDomains() {
    setState({ status: "loading" });
    try {
      const [domains, forwardingRules] = await Promise.all([
        apiClient.listDomains(),
        apiClient.listForwardingRules(),
      ]);
      setState({ status: "ready", domains, forwardingRules });
    } catch (error) {
      setState({
        status: "error",
        message:
          error instanceof Error ? error.message : "Unable to load domains",
      });
    }
  }

  useEffect(() => {
    void loadDomains();
  }, [apiClient]);

  if (state.status === "loading") {
    return (
      <section className="admin-panel" aria-labelledby="routing-title">
        <Header />
        <div className="empty-state" role="status">
          Loading routing
        </div>
      </section>
    );
  }

  if (state.status === "error") {
    return (
      <section className="admin-panel" aria-labelledby="routing-title">
        <Header />
        <div className="error-state" role="alert">
          {state.message}
        </div>
      </section>
    );
  }

  return (
    <section className="admin-panel" aria-labelledby="routing-title">
      <Header />
      {actionError ? (
        <div className="error-state compact-error" role="alert">
          {actionError}
        </div>
      ) : null}
      <div className="domain-list">
        {state.domains.map((domain) => (
          <article className="domain-row" key={domain.domain_name}>
            <header className="domain-header">
              <div>
                <h2>{domain.domain_name}</h2>
                <span>{domain.active ? "Active" : "Inactive"}</span>
              </div>
              <button
                className="icon-button"
                type="button"
                title={domain.active ? "Deactivate domain" : "Activate domain"}
                aria-label={
                  domain.active ? "Deactivate domain" : "Activate domain"
                }
                onClick={() =>
                  void updateDomain(domain.domain_name, {
                    active: !domain.active,
                  })
                }
              >
                {domain.active ? (
                  <Power aria-hidden="true" size={17} />
                ) : (
                  <PowerOff aria-hidden="true" size={17} />
                )}
              </button>
            </header>

            <label className="field-control">
              <span>Routing policy for {domain.domain_name}</span>
              <select
                value={domain.routing_policy}
                onChange={(event) =>
                  void updateDomain(domain.domain_name, {
                    routing_policy: event.currentTarget.value as RoutingPolicy,
                  })
                }
              >
                <option value="allowlist">allowlist</option>
                <option value="catchall">catchall</option>
              </select>
            </label>

            <form
              className="address-add-form"
              onSubmit={(event) => {
                event.preventDefault();
                void addAddress(domain.domain_name);
              }}
            >
              <label className="field-control">
                <span>Add address for {domain.domain_name}</span>
                <input
                  value={draftLocalParts[domain.domain_name] ?? ""}
                  onChange={(event) => {
                    const value = event.currentTarget.value;
                    setDraftLocalParts((current) => ({
                      ...current,
                      [domain.domain_name]: value,
                    }));
                  }}
                />
              </label>
              <button className="secondary-button" type="submit">
                <Plus aria-hidden="true" size={15} />
                Add address
              </button>
            </form>

            <ul
              className="address-list"
              aria-label={`Addresses for ${domain.domain_name}`}
            >
              {domain.addresses.map((address) => (
                <li key={address.local_part}>
                  <span>{address.local_part}</span>
                  <strong>{address.active ? "active" : "inactive"}</strong>
                  <button
                    className="icon-button"
                    type="button"
                    title={`Deactivate ${address.local_part}`}
                    aria-label={`Deactivate ${address.local_part}`}
                    disabled={!address.active}
                    onClick={() =>
                      void deactivateAddress(
                        domain.domain_name,
                        address.local_part,
                      )
                    }
                  >
                    <Trash2 aria-hidden="true" size={15} />
                  </button>
                </li>
              ))}
            </ul>

            <section
              className="forwarding-rules"
              aria-label={`Forwarding rules for ${domain.domain_name}`}
            >
              <h3>Forwarding rules</h3>
              <form
                className="forwarding-add-form"
                onSubmit={(event) => {
                  event.preventDefault();
                  void addForwardingRule(domain);
                }}
              >
                <label className="field-control">
                  <span>Source address</span>
                  <select
                    value={
                      draftForwarding[domain.domain_name]?.local_part ??
                      domain.addresses[0]?.local_part ??
                      ""
                    }
                    onChange={(event) => {
                      const localPart = event.currentTarget.value;
                      setDraftForwarding((current) => ({
                        ...current,
                        [domain.domain_name]: {
                          local_part: localPart,
                          target_address:
                            current[domain.domain_name]?.target_address ?? "",
                        },
                      }));
                    }}
                  >
                    {domain.addresses.map((address) => (
                      <option
                        key={address.local_part}
                        value={address.local_part}
                      >
                        {address.local_part}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="field-control">
                  <span>Forward to</span>
                  <input
                    value={
                      draftForwarding[domain.domain_name]?.target_address ?? ""
                    }
                    onChange={(event) => {
                      const targetAddress = event.currentTarget.value;
                      setDraftForwarding((current) => ({
                        ...current,
                        [domain.domain_name]: {
                          local_part:
                            current[domain.domain_name]?.local_part ??
                            domain.addresses[0]?.local_part ??
                            "",
                          target_address: targetAddress,
                        },
                      }));
                    }}
                  />
                </label>
                <button className="secondary-button" type="submit">
                  <Plus aria-hidden="true" size={15} />
                  Add forwarding
                </button>
              </form>
              <ul className="forwarding-rule-list">
                {state.forwardingRules
                  .filter((rule) => rule.domain_name === domain.domain_name)
                  .map((rule) => (
                    <li key={rule.id}>
                      <span>
                        {rule.local_part}@{rule.domain_name}
                      </span>
                      <strong>{rule.target_address}</strong>
                      <em>{rule.active ? "active" : "inactive"}</em>
                      <button
                        className="icon-button"
                        type="button"
                        title={`Deactivate forwarding rule ${rule.target_address}`}
                        aria-label={`Deactivate forwarding rule ${rule.target_address}`}
                        disabled={!rule.active}
                        onClick={() => void deactivateForwardingRule(rule.id)}
                      >
                        <Trash2 aria-hidden="true" size={15} />
                      </button>
                    </li>
                  ))}
              </ul>
            </section>
          </article>
        ))}
      </div>
    </section>
  );

  async function updateDomain(
    domainName: string,
    request: { routing_policy?: RoutingPolicy; active?: boolean },
  ) {
    setActionError(undefined);
    try {
      const updated = await apiClient.updateDomain(domainName, request);
      replaceDomain(updated);
    } catch (error) {
      setActionError(
        error instanceof Error ? error.message : "Unable to update domain",
      );
    }
  }

  async function addAddress(domainName: string) {
    const localPart = (draftLocalParts[domainName] ?? "").trim();
    if (!localPart) {
      return;
    }
    setActionError(undefined);
    try {
      await apiClient.addAddress(domainName, localPart);
      setDraftLocalParts((current) => ({ ...current, [domainName]: "" }));
      await refreshDomain(domainName);
    } catch (error) {
      setActionError(
        error instanceof Error ? error.message : "Unable to add address",
      );
    }
  }

  async function deactivateAddress(domainName: string, localPart: string) {
    setActionError(undefined);
    try {
      await apiClient.deactivateAddress(domainName, localPart);
      await refreshDomain(domainName);
    } catch (error) {
      setActionError(
        error instanceof Error ? error.message : "Unable to update address",
      );
    }
  }

  async function addForwardingRule(domain: DomainConfig) {
    const draft = draftForwarding[domain.domain_name];
    const localPart =
      draft?.local_part || domain.addresses[0]?.local_part || "";
    const targetAddress = draft?.target_address.trim() ?? "";
    if (!localPart || !targetAddress) {
      return;
    }
    setActionError(undefined);
    try {
      await apiClient.upsertForwardingRule({
        domain_name: domain.domain_name,
        local_part: localPart,
        target_address: targetAddress,
      });
      setDraftForwarding((current) => ({
        ...current,
        [domain.domain_name]: { local_part: localPart, target_address: "" },
      }));
      await refreshForwardingRules();
    } catch (error) {
      setActionError(
        error instanceof Error
          ? error.message
          : "Unable to update forwarding rule",
      );
    }
  }

  async function deactivateForwardingRule(ruleId: string) {
    setActionError(undefined);
    try {
      await apiClient.deactivateForwardingRule(ruleId);
      await refreshForwardingRules();
    } catch (error) {
      setActionError(
        error instanceof Error
          ? error.message
          : "Unable to update forwarding rule",
      );
    }
  }

  async function refreshDomain(domainName: string) {
    const domains = await apiClient.listDomains();
    const updated = domains.find((domain) => domain.domain_name === domainName);
    if (updated) {
      replaceDomain(updated);
    }
  }

  function replaceDomain(updated: DomainConfig) {
    setState((current) =>
      current.status === "ready"
        ? {
            ...current,
            domains: current.domains.map((domain) =>
              domain.domain_name === updated.domain_name ? updated : domain,
            ),
          }
        : current,
    );
  }

  async function refreshForwardingRules() {
    const forwardingRules = await apiClient.listForwardingRules();
    setState((current) =>
      current.status === "ready" ? { ...current, forwardingRules } : current,
    );
  }
}

function Header() {
  return (
    <header className="admin-toolbar">
      <div className="toolbar-title">
        <ShieldCheck aria-hidden="true" size={18} />
        <h1 id="routing-title">Routing policy</h1>
      </div>
    </header>
  );
}
