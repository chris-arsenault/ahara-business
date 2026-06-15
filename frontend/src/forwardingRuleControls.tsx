/* eslint-disable max-lines-per-function */
import { Plus, Trash2 } from "lucide-react";
import type { FormEvent } from "react";
import type { DomainConfig, ForwardingRule } from "./types";
import {
  firstActiveAddress,
  ruleSource,
  type ForwardingDraft,
} from "./forwardingRuleDrafts";

export function ForwardingRuleForm({
  domains,
  draft,
  onChange,
  onSubmit,
}: {
  domains: DomainConfig[];
  draft: ForwardingDraft;
  onChange: (draft: ForwardingDraft) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  const selectedDomain = domains.find(
    (domain) => domain.domain_name === draft.domain_name,
  );
  const addresses =
    selectedDomain?.addresses.filter((address) => address.active) ?? [];
  const disableAddress = draft.scope === "domain" || addresses.length === 0;
  return (
    <form className="business-form forwarding-rule-form" onSubmit={onSubmit}>
      <h2>New rule</h2>
      {domains.length === 0 ? (
        <div className="empty-state compact-empty">
          No mail domains available
        </div>
      ) : null}
      <div className="forwarding-add-form">
        <label className="field-control">
          <span>Domain</span>
          <select
            disabled={domains.length === 0}
            value={draft.domain_name}
            onChange={(event) =>
              onChange(
                draftForDomain(draft, domains, event.currentTarget.value),
              )
            }
          >
            {domains.map((domain) => (
              <option key={domain.domain_name} value={domain.domain_name}>
                {domain.domain_name}
              </option>
            ))}
          </select>
        </label>
        <label className="field-control">
          <span>Scope</span>
          <select
            disabled={domains.length === 0}
            value={draft.scope}
            onChange={(event) =>
              onChange(
                draftForScope(
                  draft,
                  addresses,
                  event.currentTarget.value as ForwardingDraft["scope"],
                ),
              )
            }
          >
            <option value="address">address</option>
            <option value="domain">domain</option>
          </select>
        </label>
        <label className="field-control">
          <span>Source address</span>
          <select
            disabled={disableAddress}
            value={draft.local_part}
            onChange={(event) =>
              onChange({ ...draft, local_part: event.currentTarget.value })
            }
          >
            {addresses.map((address) => (
              <option key={address.local_part} value={address.local_part}>
                {address.local_part}
              </option>
            ))}
          </select>
        </label>
        <label className="field-control">
          <span>Forward to</span>
          <input
            disabled={domains.length === 0}
            value={draft.target_address}
            onChange={(event) =>
              onChange({ ...draft, target_address: event.currentTarget.value })
            }
          />
        </label>
        <label className="field-control">
          <span>Sender filter</span>
          <input
            disabled={domains.length === 0}
            value={draft.sender_address}
            onChange={(event) =>
              onChange({ ...draft, sender_address: event.currentTarget.value })
            }
          />
        </label>
        <label className="field-control">
          <span>Plus tag</span>
          <input
            disabled={domains.length === 0}
            value={draft.plus_tag}
            onChange={(event) =>
              onChange({ ...draft, plus_tag: event.currentTarget.value })
            }
          />
        </label>
        <label className="checkbox-control">
          <input
            checked={draft.require_auth_pass}
            disabled={domains.length === 0}
            type="checkbox"
            onChange={(event) =>
              onChange({
                ...draft,
                require_auth_pass: event.currentTarget.checked,
              })
            }
          />
          <span>Require auth pass</span>
        </label>
        <button
          className="secondary-button"
          disabled={
            domains.length === 0 ||
            (draft.scope === "address" && !draft.local_part)
          }
          type="submit"
        >
          <Plus aria-hidden="true" size={15} />
          Add forwarding
        </button>
      </div>
    </form>
  );
}

export function ForwardingRuleList({
  rules,
  onDeactivate,
}: {
  rules: ForwardingRule[];
  onDeactivate: (ruleId: string) => Promise<void>;
}) {
  return (
    <section className="business-list forwarding-rule-manager">
      <h2>Rules</h2>
      {rules.length === 0 ? (
        <div className="empty-state compact-empty">No forwarding rules yet</div>
      ) : (
        rules.map((rule) => (
          <article key={rule.id}>
            <strong>{ruleSource(rule)}</strong>
            <span>{rule.target_address}</span>
            <small>{ruleFilterSummary(rule)}</small>
            <div className="inline-actions">
              <em>{rule.active ? "active" : "inactive"}</em>
              <button
                className="icon-button"
                type="button"
                title={`Deactivate forwarding rule ${rule.target_address}`}
                aria-label={`Deactivate forwarding rule ${rule.target_address}`}
                disabled={!rule.active}
                onClick={() => void onDeactivate(rule.id)}
              >
                <Trash2 aria-hidden="true" size={15} />
              </button>
            </div>
          </article>
        ))
      )}
    </section>
  );
}

function draftForDomain(
  draft: ForwardingDraft,
  domains: DomainConfig[],
  domainName: string,
) {
  const domain = domains.find((item) => item.domain_name === domainName);
  return {
    ...draft,
    domain_name: domainName,
    local_part: firstActiveAddress(domain),
  };
}

function draftForScope(
  draft: ForwardingDraft,
  addresses: DomainConfig["addresses"],
  scope: ForwardingDraft["scope"],
) {
  return {
    ...draft,
    local_part:
      scope === "address" && !draft.local_part
        ? (addresses[0]?.local_part ?? "")
        : draft.local_part,
    scope,
  };
}

function ruleFilterSummary(rule: ForwardingRule) {
  return [
    rule.sender_address_normalized
      ? `from ${rule.sender_address_normalized}`
      : null,
    rule.plus_tag ? `+${rule.plus_tag}` : null,
    rule.require_auth_pass ? "auth pass" : "auth optional",
  ]
    .filter(Boolean)
    .join(" / ");
}
