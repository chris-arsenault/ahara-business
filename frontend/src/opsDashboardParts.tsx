import { RefreshCw } from "lucide-react";
import type { ChangeEvent, FormEvent } from "react";
import type {
  OperationLogEvent,
  OperationSummary,
  OperationType,
  OpsQuery,
} from "./opsApi";

export type DashboardState =
  | { status: "loading" }
  | { status: "ready"; summaries: OperationSummary[] }
  | { status: "error"; message: string };

export type EventState =
  | { status: "idle" }
  | { status: "loading"; operation: OperationSummary }
  | {
      status: "ready";
      operation: OperationSummary;
      events: OperationLogEvent[];
    }
  | { status: "error"; operation: OperationSummary; message: string };

const operationTypes: Array<{ value: OperationType | ""; label: string }> = [
  { value: "", label: "All types" },
  { value: "user_interaction", label: "User" },
  { value: "polling", label: "Polling" },
  { value: "health", label: "Health" },
  { value: "background", label: "Background" },
  { value: "system", label: "System" },
];

export function OpsHeader({ onRefresh }: { onRefresh: () => void }) {
  return (
    <header className="ops-header">
      <div>
        <h1 id="ops-title">Operations</h1>
        <p>CloudWatch OTEL logs by service, operation, and interaction type.</p>
      </div>
      <button
        className="secondary-button compact-button"
        type="button"
        onClick={onRefresh}
      >
        <RefreshCw aria-hidden="true" size={15} />
        Refresh
      </button>
    </header>
  );
}

export function OpsToolbar({
  draft,
  onApply,
  onPatch,
}: {
  draft: OpsQuery;
  onApply: (event: FormEvent<HTMLFormElement>) => void;
  onPatch: (patch: OpsQuery) => void;
}) {
  function handleMinutesChange(event: ChangeEvent<HTMLSelectElement>) {
    onPatch({ minutes: Number(event.target.value) });
  }

  function handleServiceChange(service: string) {
    onPatch({ service });
  }

  function handleOperationChange(operation: string) {
    onPatch({ operation });
  }

  function handleTypeChange(event: ChangeEvent<HTMLSelectElement>) {
    onPatch({ operation_type: event.target.value });
  }

  return (
    <form className="ops-toolbar" onSubmit={onApply}>
      <label>
        <span>Window</span>
        <select
          value={String(draft.minutes ?? 60)}
          onChange={handleMinutesChange}
        >
          <option value="15">15 min</option>
          <option value="60">1 hour</option>
          <option value="240">4 hours</option>
          <option value="1440">24 hours</option>
        </select>
      </label>
      <TextFilter
        label="Service"
        placeholder="linkdrop-api"
        value={draft.service ?? ""}
        onChange={handleServiceChange}
      />
      <TextFilter
        label="Operation"
        placeholder="api.items.complete_image_upload"
        value={draft.operation ?? ""}
        onChange={handleOperationChange}
      />
      <label>
        <span>Type</span>
        <select value={draft.operation_type ?? ""} onChange={handleTypeChange}>
          {operationTypes.map((type) => (
            <option key={type.label} value={type.value}>
              {type.label}
            </option>
          ))}
        </select>
      </label>
      <button className="primary-button compact-button" type="submit">
        Apply
      </button>
    </form>
  );
}

export function OperationSummaryPanel({
  selected,
  state,
  onSelect,
}: {
  selected: OperationSummary | null;
  state: DashboardState;
  onSelect: (operation: OperationSummary) => void;
}) {
  if (state.status === "loading") {
    return <section className="ops-panel">Loading operations</section>;
  }
  if (state.status === "error") {
    return <section className="ops-panel error-state">{state.message}</section>;
  }
  if (state.summaries.length === 0) {
    return <section className="ops-panel empty-state">No operations</section>;
  }
  return (
    <section className="ops-panel" aria-label="Operation summaries">
      <div className="ops-table-header summary">
        <span>Operation</span>
        <span>Type</span>
        <span>Count</span>
        <span>Avg</span>
      </div>
      {state.summaries.map((summary) => (
        <OperationSummaryRow
          active={sameOperation(selected, summary)}
          key={summaryKey(summary)}
          onSelect={onSelect}
          summary={summary}
        />
      ))}
    </section>
  );
}

export function OperationEventsPanel({ state }: { state: EventState }) {
  if (state.status === "idle") {
    return (
      <section className="ops-panel empty-state">Select an operation</section>
    );
  }
  if (state.status === "loading") {
    return <section className="ops-panel">Loading events</section>;
  }
  if (state.status === "error") {
    return <section className="ops-panel error-state">{state.message}</section>;
  }
  return (
    <section className="ops-panel" aria-label="Operation events">
      <div className="ops-events-heading">
        <h2>{state.operation.event_name}</h2>
        <span>{state.events.length} events</span>
      </div>
      {state.events.map((event) => (
        <OperationEventRow
          event={event}
          key={`${event.timestamp}:${event.message}`}
        />
      ))}
    </section>
  );
}

function OperationSummaryRow({
  active,
  onSelect,
  summary,
}: {
  active: boolean;
  onSelect: (operation: OperationSummary) => void;
  summary: OperationSummary;
}) {
  function handleClick() {
    onSelect(summary);
  }

  return (
    <button
      className="ops-table-row summary"
      data-active={active}
      type="button"
      onClick={handleClick}
    >
      <span>
        <strong>{summary.event_name}</strong>
        <small>{summary.service_name}</small>
      </span>
      <span>{summary.operation_type}</span>
      <span>{summary.count}</span>
      <span>{formatMs(summary.avg_duration_ms)}</span>
    </button>
  );
}

function OperationEventRow({ event }: { event: OperationLogEvent }) {
  return (
    <article className="ops-event">
      <div>
        <strong>{formatTimestamp(event.timestamp)}</strong>
        <span>{formatMs(event.duration_ms)}</span>
      </div>
      <dl>
        <Detail label="type" value={event.operation_type} />
        <Detail label="path" value={event.path} />
        <Detail label="status" value={event.status_code} />
      </dl>
      <pre>{formatDetails(event.operation_details)}</pre>
    </article>
  );
}

function TextFilter({
  label,
  onChange,
  placeholder,
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  placeholder: string;
  value: string;
}) {
  function handleChange(event: ChangeEvent<HTMLInputElement>) {
    onChange(event.target.value);
  }

  return (
    <label>
      <span>{label}</span>
      <input placeholder={placeholder} value={value} onChange={handleChange} />
    </label>
  );
}

function Detail({ label, value }: { label: string; value: string }) {
  if (!value) {
    return null;
  }
  return (
    <>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </>
  );
}

function sameOperation(left: OperationSummary | null, right: OperationSummary) {
  return Boolean(left && summaryKey(left) === summaryKey(right));
}

function summaryKey(summary: OperationSummary) {
  return [
    summary.service_name,
    summary.event_name,
    summary.operation_type,
    summary.event_domain,
  ].join(":");
}

function formatMs(value: number | null) {
  return value === null ? "" : `${Math.round(value)}ms`;
}

function formatTimestamp(value: string) {
  return value ? new Date(value).toLocaleString() : "";
}

function formatDetails(value: unknown) {
  if (value === null || value === undefined) {
    return "{}";
  }
  return JSON.stringify(value, null, 2);
}
