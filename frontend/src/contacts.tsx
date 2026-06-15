/* eslint-disable max-lines-per-function, react-hooks/exhaustive-deps, react-hooks/set-state-in-effect, react-perf/jsx-no-new-function-as-prop */
import { Save, UserPlus, Users, X } from "lucide-react";
import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import type { Contact } from "./types";
import type { ContactDraft, ContactsApi, ContactsState } from "./contactsTypes";

const blankDraft: ContactDraft = {
  id: null,
  display_name: "",
  primary_address: "",
  notes: "",
};

export type { ContactsApi } from "./contactsTypes";

export function ContactsView({ apiClient }: { apiClient: ContactsApi }) {
  const [state, setState] = useState<ContactsState>({ status: "loading" });
  const [editing, setEditing] = useState<ContactDraft>(blankDraft);
  const [actionError, setActionError] = useState<string | null>(null);

  async function load() {
    setState({ status: "loading" });
    try {
      setState({ status: "ready", contacts: await apiClient.listContacts() });
    } catch (error) {
      setState({
        status: "error",
        message:
          error instanceof Error ? error.message : "Unable to load contacts",
      });
    }
  }

  useEffect(() => {
    void load();
  }, [apiClient]);

  if (state.status === "loading") {
    return <Shell body={<div className="empty-state">Loading contacts</div>} />;
  }
  if (state.status === "error") {
    return <Shell body={<div className="error-state">{state.message}</div>} />;
  }

  return (
    <Shell
      body={
        <>
          {actionError ? (
            <div className="error-state compact-error" role="alert">
              {actionError}
            </div>
          ) : null}
          <div className="business-grid contacts-grid">
            <ContactList
              contacts={state.contacts}
              onEdit={setEditing}
              onNew={() => setEditing(blankDraft)}
            />
            <ContactEditor
              draft={editing}
              onCancel={() => setEditing(blankDraft)}
              onChange={setEditing}
              onSubmit={saveContact}
            />
          </div>
        </>
      }
    />
  );

  async function saveContact(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const displayName = editing.display_name.trim();
    if (!displayName) {
      setActionError("Display name is required");
      return;
    }

    await runAction(async () => {
      const request = contactRequest(editing, displayName);
      if (editing.id) {
        await apiClient.updateContact(editing.id, request);
      } else {
        await apiClient.createContact(request);
      }
      setEditing(blankDraft);
    });
  }

  async function runAction(action: () => Promise<void>) {
    setActionError(null);
    try {
      await action();
      await load();
    } catch (error) {
      setActionError(error instanceof Error ? error.message : "Action failed");
    }
  }
}

function ContactList({
  contacts,
  onEdit,
  onNew,
}: {
  contacts: Contact[];
  onEdit: (draft: ContactDraft) => void;
  onNew: () => void;
}) {
  return (
    <section className="business-list contact-list">
      <div className="business-list-heading">
        <h2>Contacts</h2>
        <button
          className="secondary-button compact-button"
          type="button"
          onClick={onNew}
        >
          <UserPlus aria-hidden="true" size={15} />
          Add
        </button>
      </div>
      {contacts.length === 0 ? (
        <div className="empty-state compact-empty">No contacts yet</div>
      ) : (
        contacts.map((contact) => (
          <article key={contact.id}>
            <strong>{contact.display_name}</strong>
            <span>{contact.primary_address || "No primary address"}</span>
            <small>{contact.notes || "No notes"}</small>
            <button
              className="secondary-button compact-button"
              type="button"
              onClick={() => onEdit(draftFromContact(contact))}
            >
              Edit
            </button>
          </article>
        ))
      )}
    </section>
  );
}

function ContactEditor({
  draft,
  onCancel,
  onChange,
  onSubmit,
}: {
  draft: ContactDraft;
  onCancel: () => void;
  onChange: (draft: ContactDraft) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  return (
    <form className="business-form contact-editor" onSubmit={onSubmit}>
      <div className="business-list-heading">
        <h2>{draft.id ? "Edit contact" : "New contact"}</h2>
        {draft.id ? (
          <button
            className="icon-button"
            type="button"
            title="Clear editor"
            aria-label="Clear contact editor"
            onClick={onCancel}
          >
            <X aria-hidden="true" size={16} />
          </button>
        ) : null}
      </div>
      <label className="field-control">
        <span>Display name</span>
        <input
          value={draft.display_name}
          onChange={(event) =>
            onChange({ ...draft, display_name: event.currentTarget.value })
          }
        />
      </label>
      <label className="field-control">
        <span>Primary address</span>
        <input
          value={draft.primary_address}
          onChange={(event) =>
            onChange({ ...draft, primary_address: event.currentTarget.value })
          }
        />
      </label>
      <label className="field-control">
        <span>Notes</span>
        <textarea
          value={draft.notes}
          onChange={(event) =>
            onChange({ ...draft, notes: event.currentTarget.value })
          }
        />
      </label>
      <div className="inline-actions">
        <button className="primary-button" type="submit">
          <Save aria-hidden="true" size={15} />
          Save
        </button>
        <button className="secondary-button" type="button" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </form>
  );
}

function Shell({ body }: { body: ReactNode }) {
  return (
    <section className="admin-panel" aria-labelledby="contacts-title">
      <header className="admin-toolbar">
        <div className="toolbar-title">
          <Users aria-hidden="true" size={18} />
          <h1 id="contacts-title">Contacts</h1>
        </div>
      </header>
      {body}
    </section>
  );
}

function contactRequest(draft: ContactDraft, displayName: string) {
  return {
    display_name: displayName,
    primary_address: draft.primary_address.trim() || null,
    notes: draft.notes,
  };
}

function draftFromContact(contact: Contact): ContactDraft {
  return {
    id: contact.id,
    display_name: contact.display_name,
    primary_address: contact.primary_address ?? "",
    notes: contact.notes,
  };
}
