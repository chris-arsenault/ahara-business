import type { ApiClient } from "./api";
import type { DomainConfig, RoutingPolicy } from "./types";

export type { DomainConfig, UpdateDomainRequest } from "./types";

export type DomainDraft = {
  domainName: string;
  routingPolicy: RoutingPolicy;
};

export type RoutingAdminApi = Pick<
  ApiClient,
  | "listDomains"
  | "createDomain"
  | "updateDomain"
  | "addAddress"
  | "updateAddress"
  | "deactivateAddress"
>;

export type RoutingState =
  | { status: "loading" }
  | {
      status: "ready";
      domains: DomainConfig[];
    }
  | { status: "error"; message: string };

export type RetentionDrafts = Record<string, string>;
