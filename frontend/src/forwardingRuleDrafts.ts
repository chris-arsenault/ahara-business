import type { DomainConfig } from "./types";

export type ForwardingDraft = {
  domain_name: string;
  local_part: string;
  plus_tag: string;
  require_auth_pass: boolean;
  scope: "address" | "domain";
  sender_address: string;
  target_address: string;
};

export function blankForwardingDraft(domains: DomainConfig[]): ForwardingDraft {
  const domain = domains[0];
  return {
    domain_name: domain?.domain_name ?? "",
    local_part: firstActiveAddress(domain),
    plus_tag: "",
    require_auth_pass: true,
    scope: "address",
    sender_address: "",
    target_address: "",
  };
}

export function normalizeForwardingDraft(
  draft: ForwardingDraft,
  domains: DomainConfig[],
) {
  const domain = domains.find((item) => item.domain_name === draft.domain_name);
  if (!domain) {
    return blankForwardingDraft(domains);
  }
  const localPart = domain.addresses.some(
    (address) => address.active && address.local_part === draft.local_part,
  )
    ? draft.local_part
    : firstActiveAddress(domain);
  return { ...draft, local_part: localPart };
}

export function ruleSource(rule: {
  domain_name: string;
  local_part: string | null;
  rule_kind: string;
}) {
  return rule.rule_kind === "domain"
    ? `*@${rule.domain_name}`
    : `${rule.local_part}@${rule.domain_name}`;
}

export function firstActiveAddress(domain: DomainConfig | undefined) {
  return domain?.addresses.find((address) => address.active)?.local_part ?? "";
}
