import { config } from "./config";
import { authenticatedRequest, type ApiClientOptions } from "./apiCore";

export type OperationType =
  | "user_interaction"
  | "polling"
  | "health"
  | "background"
  | "system";

export type OpsQuery = Partial<{
  minutes: number;
  limit: number;
  service: string;
  operation: string;
  operation_type: OperationType | string;
}>;

export type OperationSummary = {
  service_name: string;
  event_name: string;
  event_domain: string;
  operation_type: string;
  count: number;
  avg_duration_ms: number | null;
  max_duration_ms: number | null;
};

export type OperationLogEvent = {
  timestamp: string;
  service_name: string;
  event_name: string;
  event_domain: string;
  operation_type: string;
  operation_details: unknown;
  duration_ms: number | null;
  http_method: string;
  path: string;
  status_code: string;
  message: string;
};

export type OperationsResponse = {
  minutes: number;
  log_group_count: number;
  operations: OperationSummary[];
};

export type EventsResponse = {
  minutes: number;
  log_group_count: number;
  events: OperationLogEvent[];
};

export type OpsApiSurface = {
  listOperationSummaries: (query?: OpsQuery) => Promise<OperationsResponse>;
  listOperationEvents: (query?: OpsQuery) => Promise<EventsResponse>;
};

class OpsApiClient implements OpsApiSurface {
  private readonly baseUrl: string;
  private readonly options: ApiClientOptions;

  constructor(options: ApiClientOptions) {
    this.options = options;
    this.baseUrl = (options.opsBaseUrl ?? config.opsApiBaseUrl).replace(
      /\/$/,
      "",
    );
  }

  listOperationSummaries(query: OpsQuery = {}) {
    return this.request<OperationsResponse>(
      `/api/ops/operations${queryString(queryParams(query))}`,
    );
  }

  listOperationEvents(query: OpsQuery = {}) {
    return this.request<EventsResponse>(
      `/api/ops/events${queryString(queryParams(query))}`,
    );
  }

  private request<T>(path: string) {
    return authenticatedRequest<T>({
      baseUrl: this.baseUrl,
      clientOptions: this.options,
      path,
      requestOptions: {},
    });
  }
}

export function attachOpsApi<T extends object>(
  baseClient: T,
  options: ApiClientOptions,
): T & OpsApiSurface {
  const ops = new OpsApiClient(options);
  return Object.assign(baseClient, bindOpsApi(ops));
}

function bindOpsApi(ops: OpsApiClient): OpsApiSurface {
  return {
    listOperationSummaries: (query) => ops.listOperationSummaries(query),
    listOperationEvents: (query) => ops.listOperationEvents(query),
  };
}

function queryParams(query: OpsQuery) {
  const params = new URLSearchParams();
  appendNumber(params, "minutes", query.minutes);
  appendNumber(params, "limit", query.limit);
  appendString(params, "service", query.service);
  appendString(params, "operation", query.operation);
  appendString(params, "operation_type", query.operation_type);
  return params;
}

function appendNumber(params: URLSearchParams, name: string, value?: number) {
  if (value !== undefined) {
    params.set(name, String(value));
  }
}

function appendString(params: URLSearchParams, name: string, value?: string) {
  if (value && value.trim()) {
    params.set(name, value.trim());
  }
}

function queryString(params: URLSearchParams) {
  const value = params.toString();
  return value ? `?${value}` : "";
}
