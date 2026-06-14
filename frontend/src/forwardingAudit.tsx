/* eslint-disable react-hooks/exhaustive-deps, react-hooks/set-state-in-effect, react-perf/jsx-no-new-function-as-prop */
import { useEffect, useState, type ReactNode } from "react";
import { Forward, RefreshCw } from "lucide-react";
import type { ForwardingAuditApi } from "./forwardingAuditTypes";
import type { ForwardingMessageStatus, ForwardingRuleStatus } from "./types";

type State =
  | { status: "loading" }
  | {
      status: "ready";
      rules: ForwardingRuleStatus[];
      messages: ForwardingMessageStatus[];
    }
  | { status: "error"; message: string };

export function ForwardingAuditView({
  apiClient,
}: {
  apiClient: ForwardingAuditApi;
}) {
  const [state, setState] = useState<State>({ status: "loading" });

  async function load() {
    setState({ status: "loading" });
    try {
      const [rules, messages] = await Promise.all([
        apiClient.listForwardingRuleStatuses(),
        apiClient.listForwardingMessageStatuses(100),
      ]);
      setState({ status: "ready", rules, messages });
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
        <div className="business-grid wide">
          <section className="business-list forwarding-status-list">
            <h2>Rules</h2>
            {state.rules.map((rule) => (
              <article key={rule.rule_id}>
                <strong>{ruleSource(rule)}</strong>
                <span>{rule.target_address}</span>
                <small>{rule.active ? "active" : "inactive"}</small>
                <StatusCounts status={rule} />
                {rule.last_error ? <em>{rule.last_error}</em> : null}
              </article>
            ))}
          </section>
          <section className="business-list forwarding-status-list">
            <h2>Messages</h2>
            {state.messages.map((message) => (
              <article key={message.source_message_id}>
                <strong>{message.subject || "(no subject)"}</strong>
                <span>{message.from_address}</span>
                <small>{message.matching_rule_count} rules</small>
                <StatusCounts status={message} />
                {message.last_error ? <em>{message.last_error}</em> : null}
              </article>
            ))}
          </section>
        </div>
      }
      onRefresh={() => void load()}
    />
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

function ruleSource(rule: ForwardingRuleStatus) {
  return rule.rule_kind === "domain"
    ? `*@${rule.domain_name}`
    : `${rule.local_part}@${rule.domain_name}`;
}
