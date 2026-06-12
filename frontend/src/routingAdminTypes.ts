import type { ApiClient } from "./api";
import type { DomainConfig, ForwardingRule } from "./types";

export type { DomainConfig, UpdateDomainRequest } from "./types";

export type RoutingAdminApi = Pick<
  ApiClient,
  | "listDomains"
  | "updateDomain"
  | "addAddress"
  | "updateAddress"
  | "deactivateAddress"
  | "listForwardingRules"
  | "upsertForwardingRule"
  | "deactivateForwardingRule"
>;

export type RoutingState =
  | { status: "loading" }
  | {
      status: "ready";
      domains: DomainConfig[];
      forwardingRules: ForwardingRule[];
    }
  | { status: "error"; message: string };

export type DraftForwarding = Record<
  string,
  {
    scope: "address" | "domain";
    local_part: string;
    target_address: string;
    sender_address: string;
    plus_tag: string;
    require_auth_pass: boolean;
  }
>;

export type RetentionDrafts = Record<string, string>;
