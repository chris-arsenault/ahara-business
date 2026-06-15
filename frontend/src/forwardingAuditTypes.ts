import type { ApiClient } from "./api";

export type ForwardingAuditApi = Pick<
  ApiClient,
  "listForwardingRuleStatuses" | "listForwardingMessageStatuses"
>;

export type ForwardingApi = Pick<
  ApiClient,
  | "listDomains"
  | "listForwardingRules"
  | "upsertForwardingRule"
  | "deactivateForwardingRule"
  | "listForwardingRuleStatuses"
  | "listForwardingMessageStatuses"
>;

export function isForwardingAuditApi(
  apiClient: Partial<ForwardingAuditApi>,
): apiClient is ForwardingAuditApi {
  return Boolean(
    apiClient.listForwardingRuleStatuses &&
    apiClient.listForwardingMessageStatuses,
  );
}

export function isForwardingApi(
  apiClient: Partial<ForwardingApi>,
): apiClient is ForwardingApi {
  return Boolean(
    apiClient.listDomains &&
    apiClient.listForwardingRules &&
    apiClient.upsertForwardingRule &&
    apiClient.deactivateForwardingRule &&
    apiClient.listForwardingRuleStatuses &&
    apiClient.listForwardingMessageStatuses,
  );
}
