import type { ApiClient } from "./api";

export type ForwardingAuditApi = Pick<
  ApiClient,
  "listForwardingRuleStatuses" | "listForwardingMessageStatuses"
>;

export function isForwardingAuditApi(
  apiClient: Partial<ForwardingAuditApi>,
): apiClient is ForwardingAuditApi {
  return Boolean(
    apiClient.listForwardingRuleStatuses &&
    apiClient.listForwardingMessageStatuses,
  );
}
