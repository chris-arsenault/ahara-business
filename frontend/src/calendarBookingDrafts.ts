import type { CalendarEventStatus } from "./types";

export type EventDraft = {
  title: string;
  starts_at: string;
  ends_at: string;
  timezone: string;
  status: CalendarEventStatus;
  contact_id: string;
  location: string;
  description: string;
  source_message_id: string | null;
  source_attachment_id: string | null;
};

export type BookingDraft = {
  title: string;
  starts_at: string;
  ends_at: string;
  calendar_event_id: string;
  contact_id: string;
  location: string;
  notes: string;
};

export function defaultEventDraft(): EventDraft {
  return {
    title: "",
    starts_at: localTime(1),
    ends_at: localTime(2),
    timezone: "UTC",
    status: "tentative",
    contact_id: "",
    location: "",
    description: "",
    source_message_id: null,
    source_attachment_id: null,
  };
}

export function defaultBookingDraft(): BookingDraft {
  return {
    title: "",
    starts_at: localTime(1),
    ends_at: localTime(2),
    calendar_event_id: "",
    contact_id: "",
    location: "",
    notes: "",
  };
}

export function toApiTime(value: string) {
  return new Date(value).toISOString();
}

export function fromApiTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "";
  }
  return new Date(date.getTime() - date.getTimezoneOffset() * 60_000)
    .toISOString()
    .slice(0, 16);
}

function localTime(hoursFromNow: number) {
  return new Date(Date.now() + hoursFromNow * 60 * 60 * 1000)
    .toISOString()
    .slice(0, 16);
}
