import "@testing-library/jest-dom/vitest";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ContactsView, type ContactsApi } from "./contacts";
import type { Contact, UpdateContactRequest } from "./types";

const existingContact: Contact = {
  id: "contact-1",
  display_name: "Chris",
  primary_address: "Chris@Example.Test",
  primary_address_normalized: "chris@example.test",
  notes: "existing",
};

afterEach(() => cleanup());

describe("ContactsView", () => {
  it("creates contacts", async () => {
    const user = userEvent.setup();
    const { api, calls } = apiWithContacts([]);

    render(<ContactsView apiClient={api} />);
    await screen.findByText("No contacts yet");
    await user.type(screen.getByLabelText("Display name"), "Support");
    await user.type(
      screen.getByLabelText("Primary address"),
      "Support@Ahara.IO",
    );
    await user.type(screen.getByLabelText("Notes"), "intake");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(calls).toContain(
      'create:{"display_name":"Support","primary_address":"Support@Ahara.IO","notes":"intake"}',
    );
    expect(await screen.findByText("Support")).toBeInTheDocument();
  });

  it("edits contacts and clears primary addresses", async () => {
    const user = userEvent.setup();
    const { api, calls } = apiWithContacts([existingContact]);

    render(<ContactsView apiClient={api} />);
    await user.click(await screen.findByRole("button", { name: "Edit" }));
    await user.clear(screen.getByLabelText("Primary address"));
    await user.clear(screen.getByLabelText("Notes"));
    await user.type(screen.getByLabelText("Notes"), "updated");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(calls).toContain(
      'update:contact-1:{"display_name":"Chris","primary_address":null,"notes":"updated"}',
    );
    expect(await screen.findByText("No primary address")).toBeInTheDocument();
  });
});

function apiWithContacts(initialContacts: Contact[]) {
  let contacts = structuredClone(initialContacts);
  const calls: string[] = [];
  const api: ContactsApi = {
    listContacts: async () => contacts,
    createContact: async (request) => {
      calls.push(`create:${JSON.stringify(request)}`);
      const contact = contactFromRequest(`contact-${contacts.length + 1}`, {
        display_name: request.display_name,
        notes: request.notes ?? null,
        primary_address: request.primary_address ?? null,
      });
      contacts = [...contacts, contact];
      return contact;
    },
    updateContact: async (contactId, request) => {
      calls.push(`update:${contactId}:${JSON.stringify(request)}`);
      contacts = contacts.map((contact) =>
        contact.id === contactId ? updateContact(contact, request) : contact,
      );
      const contact = contacts.find((item) => item.id === contactId);
      if (!contact) {
        throw new Error("not found");
      }
      return contact;
    },
  };
  return { api, calls };
}

function contactFromRequest(
  id: string,
  request: {
    display_name: string;
    notes: string | null;
    primary_address: string | null;
  },
): Contact {
  return {
    id,
    display_name: request.display_name,
    primary_address: request.primary_address ?? null,
    primary_address_normalized: request.primary_address?.toLowerCase() ?? null,
    notes: request.notes ?? "",
  };
}

function updateContact(
  contact: Contact,
  request: UpdateContactRequest,
): Contact {
  return {
    ...contact,
    display_name: request.display_name ?? contact.display_name,
    notes: request.notes ?? contact.notes,
    primary_address:
      "primary_address" in request
        ? (request.primary_address ?? null)
        : contact.primary_address,
    primary_address_normalized:
      "primary_address" in request
        ? (request.primary_address?.toLowerCase() ?? null)
        : contact.primary_address_normalized,
  };
}
