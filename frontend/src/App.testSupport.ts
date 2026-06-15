import type { ContactsApi } from "./contacts";
import type { ForwardingApi } from "./forwardingAuditTypes";
import type { MailboxApi } from "./mailbox";
import type { Contact, ForwardingRule, MailboxMessageSummary } from "./types";

export function contactsApi(
  message: MailboxMessageSummary,
): MailboxApi & ContactsApi {
  return {
    fetchMailboxMessages: async () => [message],
    listContacts: async () => [contact],
    createContact: async (request) => ({
      ...contact,
      display_name: request.display_name,
      notes: request.notes ?? "",
      primary_address: request.primary_address ?? null,
      primary_address_normalized:
        request.primary_address?.toLowerCase() ?? null,
    }),
    updateContact: async (_contactId, request) => ({ ...contact, ...request }),
  };
}

export function forwardingApi(
  message: MailboxMessageSummary,
): MailboxApi & ForwardingApi {
  return {
    fetchMailboxMessages: async () => [message],
    listDomains: async () => [
      {
        domain_name: "ahara.io",
        routing_policy: "allowlist",
        active: true,
        raw_retention_days: 365,
        addresses: [
          { local_part: "chris", active: true, raw_retention_days: null },
        ],
      },
    ],
    listForwardingRules: async () => [],
    upsertForwardingRule: async (request) =>
      forwardingRule(request.target_address),
    deactivateForwardingRule: async () =>
      forwardingRule("target@example.com", false),
    listForwardingRuleStatuses: async () => [],
    listForwardingMessageStatuses: async () => [],
  };
}

function forwardingRule(targetAddress: string, active = true): ForwardingRule {
  return {
    id: "rule-1",
    rule_kind: "address",
    domain_name: "ahara.io",
    local_part: "chris",
    address_id: "ahara.io:chris",
    target_address: targetAddress,
    target_address_normalized: targetAddress.toLowerCase(),
    sender_address_normalized: null,
    plus_tag: null,
    require_auth_pass: true,
    active,
    created_at: null,
    updated_at: null,
  };
}

const contact: Contact = {
  id: "contact-1",
  display_name: "Chris",
  primary_address: "chris@example.test",
  primary_address_normalized: "chris@example.test",
  notes: "existing",
};
