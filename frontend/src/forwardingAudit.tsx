/* eslint-disable max-lines-per-function, react-hooks/exhaustive-deps, react-hooks/set-state-in-effect, react-perf/jsx-no-new-function-as-prop */
import { Forward, RefreshCw } from "lucide-react";
import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import type { ForwardingApi } from "./forwardingAuditTypes";
import {
  ForwardingRuleForm,
  ForwardingRuleList,
} from "./forwardingRuleControls";
import {
  blankForwardingDraft,
  normalizeForwardingDraft,
  ruleSource,
  type ForwardingDraft,
} from "./forwardingRuleDrafts";
import type {
  DomainConfig,
  ForwardingMessageStatus,
  ForwardingRule,
  ForwardingRuleStatus,
} from "./types";

type State =
  | { status: "loading" }
  | {
      status: "ready";
      domains: DomainConfig[];
      messages: ForwardingMessageStatus[];
      rules: ForwardingRule[];
      ruleStatuses: ForwardingRuleStatus[];
    }
  | { status: "error"; message: string };

export function ForwardingView({ apiClient }: { apiClient: ForwardingApi }) {
  const [state, setState] = useState<State>({ status: "loading" });
  const [draft, setDraft] = useState<ForwardingDraft>(blankForwardingDraft([]));
  const [actionError, setActionError] = useState<string | null>(null);

  async function load(showLoading = true) {
    if (showLoading) {
      setState({ status: "loading" });
    }
    try {
      const next = await loadForwarding(apiClient);
      setState(next);
      setDraft((current) => normalizeForwardingDraft(current, next.domains));
    } catch (error) {
      setState({
        status: "error",
        message:
          error instanceof Error ? error.message : "Unable to load forwarding",
      });
    }
  }

  useEffect(() => {
    void load();
  }, [apiClient]);

  if (state.status === "loading") {
    return (
      <Shell
        body={<div className="empty-state">Loading forwarding</div>}
        onRefresh={null}
      />
    );
  }
  if (state.status === "error") {
    return (
      <Shell
        body={<div className="error-state">{state.message}</div>}
        onRefresh={null}
      />
    );
  }

  return (
    <Shell
      body={
        <>
          {actionError ? (
            <div className="error-state compact-error" role="alert">
              {actionError}
            </div>
          ) : null}
          <div className="business-grid wide forwarding-grid">
            <ForwardingRuleForm
              domains={state.domains}
              draft={draft}
              onChange={setDraft}
              onSubmit={saveRule}
            />
            <ForwardingRuleList
              rules={state.rules}
              onDeactivate={deactivateRule}
            />
            <RuleStatusList rules={state.ruleStatuses} />
            <MessageStatusList messages={state.messages} />
          </div>
        </>
      }
      onRefresh={() => void load(false)}
    />
  );

  async function saveRule(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const targetAddress = draft.target_address.trim();
    if (!targetAddress) {
      setActionError("Forward to is required");
      return;
    }
    if (draft.scope === "address" && !draft.local_part) {
      setActionError("Source address is required");
      return;
    }

    await runAction(async () => {
      await apiClient.upsertForwardingRule({
        domain_name: draft.domain_name,
        local_part: draft.scope === "address" ? draft.local_part : null,
        plus_tag: draft.plus_tag.trim() || null,
        require_auth_pass: draft.require_auth_pass,
        sender_address: draft.sender_address.trim() || null,
        target_address: targetAddress,
      });
      setDraft((current) => ({ ...current, target_address: "" }));
    });
  }

  async function deactivateRule(ruleId: string) {
    await runAction(async () => {
      await apiClient.deactivateForwardingRule(ruleId);
    });
  }

  async function runAction(action: () => Promise<void>) {
    setActionError(null);
    try {
      await action();
      await load(false);
    } catch (error) {
      setActionError(error instanceof Error ? error.message : "Action failed");
    }
  }
}

function RuleStatusList({ rules }: { rules: ForwardingRuleStatus[] }) {
  return (
    <section className="business-list forwarding-status-list">
      <h2>Rule status</h2>
      {rules.length === 0 ? (
        <div className="empty-state compact-empty">
          No forwarding activity yet
        </div>
      ) : (
        rules.map((rule) => (
          <article key={rule.rule_id}>
            <strong>{ruleSource(rule)}</strong>
            <span>{rule.target_address}</span>
            <small>{rule.active ? "active" : "inactive"}</small>
            <StatusCounts status={rule} />
            {rule.last_error ? <em>{rule.last_error}</em> : null}
          </article>
        ))
      )}
    </section>
  );
}

function MessageStatusList({
  messages,
}: {
  messages: ForwardingMessageStatus[];
}) {
  return (
    <section className="business-list forwarding-status-list">
      <h2>Message status</h2>
      {messages.length === 0 ? (
        <div className="empty-state compact-empty">No matched messages yet</div>
      ) : (
        messages.map((message) => (
          <article key={message.source_message_id}>
            <strong>{message.subject || "(no subject)"}</strong>
            <span>{message.from_address}</span>
            <small>{message.matching_rule_count} rules</small>
            <StatusCounts status={message} />
            {message.last_error ? <em>{message.last_error}</em> : null}
          </article>
        ))
      )}
    </section>
  );
}

function Shell({
  body,
  onRefresh,
}: {
  body: ReactNode;
  onRefresh: (() => void) | null;
}) {
  return (
    <section className="admin-panel" aria-labelledby="forwarding-title">
      <header className="admin-toolbar">
        <div className="toolbar-title">
          <Forward aria-hidden="true" size={18} />
          <h1 id="forwarding-title">Forwarding</h1>
        </div>
        {onRefresh ? (
          <button
            className="icon-button"
            type="button"
            title="Refresh"
            aria-label="Refresh forwarding"
            onClick={onRefresh}
          >
            <RefreshCw aria-hidden="true" size={16} />
          </button>
        ) : null}
      </header>
      {body}
    </section>
  );
}

function StatusCounts({
  status,
}: {
  status: Pick<
    ForwardingRuleStatus,
    | "queued_count"
    | "sending_count"
    | "sent_count"
    | "failed_count"
    | "bounced_count"
    | "complained_count"
  >;
}) {
  return (
    <dl className="status-counts">
      <div>
        <dt>queued</dt>
        <dd>{status.queued_count}</dd>
      </div>
      <div>
        <dt>sent</dt>
        <dd>{status.sent_count}</dd>
      </div>
      <div>
        <dt>failed</dt>
        <dd>
          {status.failed_count + status.bounced_count + status.complained_count}
        </dd>
      </div>
      <div>
        <dt>sending</dt>
        <dd>{status.sending_count}</dd>
      </div>
    </dl>
  );
}

async function loadForwarding(
  apiClient: ForwardingApi,
): Promise<Extract<State, { status: "ready" }>> {
  const [domains, rules, ruleStatuses, messages] = await Promise.all([
    apiClient.listDomains(),
    apiClient.listForwardingRules(),
    apiClient.listForwardingRuleStatuses(),
    apiClient.listForwardingMessageStatuses(100),
  ]);
  return { status: "ready", domains, messages, rules, ruleStatuses };
}
