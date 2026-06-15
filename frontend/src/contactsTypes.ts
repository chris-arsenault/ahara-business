import type { ApiClient } from "./api";
import type { Contact } from "./types";

export type ContactsApi = Pick<
  ApiClient,
  "listContacts" | "createContact" | "updateContact"
>;

export type ContactsState =
  | { status: "loading" }
  | { status: "ready"; contacts: Contact[] }
  | { status: "error"; message: string };

export type ContactDraft = {
  id: string | null;
  display_name: string;
  primary_address: string;
  notes: string;
};
