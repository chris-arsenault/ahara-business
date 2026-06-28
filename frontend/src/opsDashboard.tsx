import { useEffect, useMemo, useState, type FormEvent } from "react";
import type { OperationSummary, OpsApiSurface, OpsQuery } from "./opsApi";
import {
  OperationEventsPanel,
  OperationSummaryPanel,
  OpsHeader,
  OpsToolbar,
  type DashboardState,
  type EventState,
} from "./opsDashboardParts";
import "./opsDashboard.css";

export type OpsDashboardApi = OpsApiSurface;

export function OpsDashboardView({
  apiClient,
}: {
  apiClient: OpsDashboardApi;
}) {
  const [draft, setDraft] = useState<OpsQuery>(() => defaultQuery());
  const [query, setQuery] = useState<OpsQuery>(() => defaultQuery());
  const [state, setState] = useState<DashboardState>({ status: "loading" });
  const [events, setEvents] = useState<EventState>({ status: "idle" });

  useEffect(() => {
    void loadSummaries(apiClient, query, setState);
  }, [apiClient, query]);

  const selectedOperation = useMemo(
    () => selectedEventOperation(events),
    [events],
  );

  return (
    <section className="admin-panel ops-shell" aria-labelledby="ops-title">
      <OpsHeader onRefresh={refreshSummaries} />
      <OpsToolbar draft={draft} onApply={applyFilters} onPatch={patchDraft} />
      <div className="ops-grid">
        <OperationSummaryPanel
          selected={selectedOperation}
          state={state}
          onSelect={selectOperation}
        />
        <OperationEventsPanel state={events} />
      </div>
    </section>
  );

  function patchDraft(patch: OpsQuery) {
    setDraft((current) => ({ ...current, ...patch }));
  }

  function applyFilters(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setQuery(cleanQuery(draft));
    setEvents({ status: "idle" });
  }

  function refreshSummaries() {
    void loadSummaries(apiClient, query, setState);
  }

  function selectOperation(operation: OperationSummary) {
    void loadEvents(apiClient, query, operation, setEvents);
  }
}

async function loadSummaries(
  apiClient: OpsDashboardApi,
  query: OpsQuery,
  setState: (state: DashboardState) => void,
) {
  setState({ status: "loading" });
  try {
    const response = await apiClient.listOperationSummaries(query);
    setState({ status: "ready", summaries: response.operations });
  } catch (error) {
    setState({
      status: "error",
      message:
        error instanceof Error ? error.message : "Unable to load operations",
    });
  }
}

async function loadEvents(
  apiClient: OpsDashboardApi,
  query: OpsQuery,
  operation: OperationSummary,
  setEvents: (state: EventState) => void,
) {
  setEvents({ status: "loading", operation });
  try {
    const response = await apiClient.listOperationEvents(
      eventQuery(query, operation),
    );
    setEvents({ status: "ready", operation, events: response.events });
  } catch (error) {
    setEvents({
      status: "error",
      operation,
      message: error instanceof Error ? error.message : "Unable to load events",
    });
  }
}

function selectedEventOperation(events: EventState) {
  if (
    events.status === "loading" ||
    events.status === "ready" ||
    events.status === "error"
  ) {
    return events.operation;
  }
  return null;
}

function eventQuery(query: OpsQuery, operation: OperationSummary) {
  return cleanQuery({
    ...query,
    limit: 50,
    service: operation.service_name,
    operation: operation.event_name,
    operation_type: operation.operation_type,
  });
}

function defaultQuery(): OpsQuery {
  return { minutes: 60, service: "linkdrop-api" };
}

function cleanQuery(query: OpsQuery): OpsQuery {
  return {
    minutes: query.minutes ?? 60,
    limit: query.limit,
    service: cleanString(query.service),
    operation: cleanString(query.operation),
    operation_type: cleanString(query.operation_type),
  };
}

function cleanString(value?: string) {
  return value?.trim() || undefined;
}
