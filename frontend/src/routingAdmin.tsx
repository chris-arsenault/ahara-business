/* eslint-disable max-lines-per-function, react-hooks/exhaustive-deps, react-hooks/set-state-in-effect */
import { useEffect, useState } from "react";
import { Plus, Power, PowerOff, ShieldCheck, Trash2 } from "lucide-react";
import type {
  DomainConfig,
  DraftForwarding,
  RetentionDrafts,
  RoutingAdminApi,
  RoutingState,
  UpdateDomainRequest,
} from "./routingAdminTypes";

export type { RoutingAdminApi } from "./routingAdminTypes";

export function RoutingAdmin({ apiClient }: { apiClient: RoutingAdminApi }) {
  const [state, setState] = useState<RoutingState>({ status: "loading" });
  const [draftLocalParts, setDraftLocalParts] = useState<
    Record<string, string>
  >({});
  const [draftForwarding, setDraftForwarding] = useState<DraftForwarding>({});
  const [retentionDrafts, setRetentionDrafts] = useState<RetentionDrafts>({});
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

  function updateRetentionDraft(key: string, value: string) {
    setRetentionDrafts((current) => ({
      ...current,
      [key]: value,
    }));
  }

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
                    routing_policy: event.currentTarget
                      .value as UpdateDomainRequest["routing_policy"],
                  })
                }
              >
                <option value="allowlist">allowlist</option>
                <option value="catchall">catchall</option>
              </select>
            </label>

            <div className="retention-controls">
              <label className="field-control">
                <span>Raw retention days</span>
                <input
                  inputMode="numeric"
                  value={
                    retentionDrafts[domain.domain_name] ??
                    String(domain.raw_retention_days ?? "")
                  }
                  onChange={(event) =>
                    updateRetentionDraft(
                      domain.domain_name,
                      event.currentTarget.value,
                    )
                  }
                />
              </label>
              <button
                className="secondary-button"
                type="button"
                onClick={() => void updateDomainRetention(domain.domain_name)}
              >
                Save retention
              </button>
            </div>

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
                  <label className="field-control compact-field">
                    <span>Raw retention</span>
                    <input
                      aria-label={`Raw retention days for ${address.local_part}@${domain.domain_name}`}
                      inputMode="numeric"
                      value={
                        retentionDrafts[
                          addressRetentionKey(
                            domain.domain_name,
                            address.local_part,
                          )
                        ] ?? String(address.raw_retention_days ?? "")
                      }
                      onChange={(event) =>
                        updateRetentionDraft(
                          addressRetentionKey(
                            domain.domain_name,
                            address.local_part,
                          ),
                          event.currentTarget.value,
                        )
                      }
                    />
                  </label>
                  <button
                    className="secondary-button compact-button"
                    type="button"
                    onClick={() =>
                      void updateAddressRetention(
                        domain.domain_name,
                        address.local_part,
                      )
                    }
                  >
                    Save
                  </button>
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
                  <span>Scope</span>
                  <select
                    value={forwardDraft(domain).scope}
                    onChange={(event) =>
                      updateForwardDraft(domain.domain_name, {
                        scope: event.currentTarget.value as
                          | "address"
                          | "domain",
                      })
                    }
                  >
                    <option value="address">address</option>
                    <option value="domain">domain</option>
                  </select>
                </label>
                <label className="field-control">
                  <span>Source address</span>
                  <select
                    disabled={forwardDraft(domain).scope === "domain"}
                    value={forwardDraft(domain).local_part}
                    onChange={(event) => {
                      const localPart = event.currentTarget.value;
                      updateForwardDraft(domain.domain_name, {
                        local_part: localPart,
                      });
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
                    value={forwardDraft(domain).target_address}
                    onChange={(event) => {
                      updateForwardDraft(domain.domain_name, {
                        target_address: event.currentTarget.value,
                      });
                    }}
                  />
                </label>
                <label className="field-control">
                  <span>Sender filter</span>
                  <input
                    value={forwardDraft(domain).sender_address}
                    onChange={(event) =>
                      updateForwardDraft(domain.domain_name, {
                        sender_address: event.currentTarget.value,
                      })
                    }
                  />
                </label>
                <label className="field-control">
                  <span>Plus tag</span>
                  <input
                    value={forwardDraft(domain).plus_tag}
                    onChange={(event) =>
                      updateForwardDraft(domain.domain_name, {
                        plus_tag: event.currentTarget.value,
                      })
                    }
                  />
                </label>
                <label className="checkbox-control">
                  <input
                    checked={forwardDraft(domain).require_auth_pass}
                    type="checkbox"
                    onChange={(event) =>
                      updateForwardDraft(domain.domain_name, {
                        require_auth_pass: event.currentTarget.checked,
                      })
                    }
                  />
                  <span>Require auth pass</span>
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
                        {rule.rule_kind === "domain"
                          ? `*@${rule.domain_name}`
                          : `${rule.local_part}@${rule.domain_name}`}
                      </span>
                      <strong>{rule.target_address}</strong>
                      {rule.sender_address_normalized ? (
                        <small>{rule.sender_address_normalized}</small>
                      ) : null}
                      {rule.plus_tag ? <small>+{rule.plus_tag}</small> : null}
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
    request: UpdateDomainRequest,
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

  async function updateDomainRetention(domainName: string) {
    const parsed = parseRetentionDays(retentionDrafts[domainName] ?? "");
    if (parsed.status === "invalid") {
      setActionError(parsed.message);
      return;
    }
    await updateDomain(domainName, { raw_retention_days: parsed.days });
  }

  async function updateAddressRetention(domainName: string, localPart: string) {
    const parsed = parseRetentionDays(
      retentionDrafts[addressRetentionKey(domainName, localPart)] ?? "",
    );
    if (parsed.status === "invalid") {
      setActionError(parsed.message);
      return;
    }
    setActionError(undefined);
    try {
      await apiClient.updateAddress(domainName, localPart, {
        raw_retention_days: parsed.days,
      });
      await refreshDomain(domainName);
    } catch (error) {
      setActionError(
        error instanceof Error ? error.message : "Unable to update address",
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
    const draft = forwardDraft(domain);
    const targetAddress = draft.target_address.trim();
    if (!targetAddress || (draft.scope === "address" && !draft.local_part)) {
      return;
    }
    setActionError(undefined);
    try {
      await apiClient.upsertForwardingRule({
        domain_name: domain.domain_name,
        local_part: draft.scope === "address" ? draft.local_part : null,
        target_address: targetAddress,
        sender_address: draft.sender_address.trim() || null,
        plus_tag: draft.plus_tag.trim() || null,
        require_auth_pass: draft.require_auth_pass,
      });
      setDraftForwarding((current) => ({
        ...current,
        [domain.domain_name]: { ...draft, target_address: "" },
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

  function forwardDraft(domain: DomainConfig) {
    return (
      draftForwarding[domain.domain_name] ?? {
        scope: "address" as const,
        local_part: domain.addresses[0]?.local_part ?? "",
        target_address: "",
        sender_address: "",
        plus_tag: "",
        require_auth_pass: true,
      }
    );
  }

  function updateForwardDraft(
    domainName: string,
    patch: Partial<DraftForwarding[string]>,
  ) {
    setDraftForwarding((current) => {
      const draft = current[domainName] ?? {
        scope: "address",
        local_part: "",
        target_address: "",
        sender_address: "",
        plus_tag: "",
        require_auth_pass: true,
      };
      return {
        ...current,
        [domainName]: {
          ...draft,
          ...patch,
        },
      };
    });
  }
}

function addressRetentionKey(domainName: string, localPart: string) {
  return `${domainName}:${localPart}`;
}

function parseRetentionDays(
  value: string,
):
  | { status: "valid"; days: number | null }
  | { status: "invalid"; message: string } {
  const rawValue = value.trim();
  const days = rawValue ? Number(rawValue) : null;
  if (rawValue && !Number.isInteger(days)) {
    return {
      status: "invalid",
      message: "Raw retention days must be a whole number",
    };
  }
  return { status: "valid", days };
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
