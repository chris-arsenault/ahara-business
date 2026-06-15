/* eslint-disable max-lines-per-function, react-hooks/exhaustive-deps, react-hooks/set-state-in-effect */
import { useEffect, useState } from "react";
import { Plus, Power, PowerOff, ShieldCheck, Trash2 } from "lucide-react";
import type {
  DomainDraft,
  DomainConfig,
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
  const [domainDraft, setDomainDraft] = useState<DomainDraft>({
    domainName: "",
    routingPolicy: "allowlist",
  });
  const [retentionDrafts, setRetentionDrafts] = useState<RetentionDrafts>({});
  const [actionError, setActionError] = useState<string>();

  async function loadDomains() {
    setState({ status: "loading" });
    try {
      setState({ status: "ready", domains: await apiClient.listDomains() });
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
      <DomainCreateForm
        draft={domainDraft}
        onChange={setDomainDraft}
        onSubmit={createDomain}
      />
      {state.domains.length === 0 ? (
        <div className="empty-state">No mail domains configured</div>
      ) : (
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
                  title={
                    domain.active ? "Deactivate domain" : "Activate domain"
                  }
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
                <span>Accepted recipient policy for {domain.domain_name}</span>
                <select
                  value={domain.routing_policy}
                  onChange={(event) =>
                    void updateDomain(domain.domain_name, {
                      routing_policy: event.currentTarget
                        .value as UpdateDomainRequest["routing_policy"],
                    })
                  }
                >
                  <option value="allowlist">Only listed addresses</option>
                  <option value="catchall">Every address on domain</option>
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

              {domain.addresses.length === 0 ? (
                <div className="empty-state compact-empty">
                  No accepted addresses
                </div>
              ) : (
                <AddressList
                  domain={domain}
                  retentionDrafts={retentionDrafts}
                  onDeactivate={deactivateAddress}
                  onRetentionChange={updateRetentionDraft}
                  onRetentionSave={updateAddressRetention}
                />
              )}
            </article>
          ))}
        </div>
      )}
    </section>
  );

  async function updateDomain(
    domainName: string,
    request: UpdateDomainRequest,
  ) {
    setActionError(undefined);
    try {
      const updated = await apiClient.updateDomain(domainName, request);
      upsertDomain(updated);
    } catch (error) {
      setActionError(
        error instanceof Error ? error.message : "Unable to update domain",
      );
    }
  }

  async function createDomain() {
    const domainName = domainDraft.domainName.trim();
    if (!domainName) {
      return;
    }
    setActionError(undefined);
    try {
      const created = await apiClient.createDomain({
        domain_name: domainName,
        routing_policy: domainDraft.routingPolicy,
      });
      setDomainDraft({ domainName: "", routingPolicy: "allowlist" });
      upsertDomain(created);
    } catch (error) {
      setActionError(
        error instanceof Error ? error.message : "Unable to add mail domain",
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

  async function refreshDomain(domainName: string) {
    const domains = await apiClient.listDomains();
    const updated = domains.find((domain) => domain.domain_name === domainName);
    if (updated) {
      upsertDomain(updated);
    }
  }

  function upsertDomain(updated: DomainConfig) {
    setState((current) =>
      current.status === "ready"
        ? {
            ...current,
            domains: sortDomains(
              current.domains.some(
                (domain) => domain.domain_name === updated.domain_name,
              )
                ? current.domains.map((domain) =>
                    domain.domain_name === updated.domain_name
                      ? updated
                      : domain,
                  )
                : [...current.domains, updated],
            ),
          }
        : current,
    );
  }
}

function DomainCreateForm({
  draft,
  onChange,
  onSubmit,
}: {
  draft: DomainDraft;
  onChange: (draft: DomainDraft) => void;
  onSubmit: () => Promise<void>;
}) {
  return (
    <form
      className="domain-create-form"
      onSubmit={(event) => {
        event.preventDefault();
        void onSubmit();
      }}
    >
      <label className="field-control">
        <span>Mail domain</span>
        <input
          placeholder="ahara.io"
          value={draft.domainName}
          onChange={(event) =>
            onChange({ ...draft, domainName: event.currentTarget.value })
          }
        />
      </label>
      <label className="field-control">
        <span>Accepted recipient policy</span>
        <select
          value={draft.routingPolicy}
          onChange={(event) =>
            onChange({
              ...draft,
              routingPolicy: event.currentTarget
                .value as DomainDraft["routingPolicy"],
            })
          }
        >
          <option value="allowlist">Only listed addresses</option>
          <option value="catchall">Every address on domain</option>
        </select>
      </label>
      <button className="secondary-button" type="submit">
        <Plus aria-hidden="true" size={15} />
        Add mail domain
      </button>
    </form>
  );
}

function AddressList({
  domain,
  retentionDrafts,
  onDeactivate,
  onRetentionChange,
  onRetentionSave,
}: {
  domain: DomainConfig;
  retentionDrafts: RetentionDrafts;
  onDeactivate: (domainName: string, localPart: string) => Promise<void>;
  onRetentionChange: (key: string, value: string) => void;
  onRetentionSave: (domainName: string, localPart: string) => Promise<void>;
}) {
  return (
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
                  addressRetentionKey(domain.domain_name, address.local_part)
                ] ?? String(address.raw_retention_days ?? "")
              }
              onChange={(event) =>
                onRetentionChange(
                  addressRetentionKey(domain.domain_name, address.local_part),
                  event.currentTarget.value,
                )
              }
            />
          </label>
          <button
            className="secondary-button compact-button"
            type="button"
            onClick={() =>
              void onRetentionSave(domain.domain_name, address.local_part)
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
              void onDeactivate(domain.domain_name, address.local_part)
            }
          >
            <Trash2 aria-hidden="true" size={15} />
          </button>
        </li>
      ))}
    </ul>
  );
}

function addressRetentionKey(domainName: string, localPart: string) {
  return `${domainName}:${localPart}`;
}

function sortDomains(domains: DomainConfig[]) {
  return [...domains].sort((left, right) =>
    left.domain_name.localeCompare(right.domain_name),
  );
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
        <h1 id="routing-title">Mail routing</h1>
      </div>
    </header>
  );
}
