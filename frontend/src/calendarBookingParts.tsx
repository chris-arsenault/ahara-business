/* eslint-disable react-perf/jsx-no-new-function-as-prop */
import { CalendarDays, RefreshCw } from "lucide-react";
import type { ReactNode } from "react";
import type {
  Booking,
  BookingStatus,
  CalendarEvent,
  CalendarEventStatus,
  Contact,
  IcsCandidate,
} from "./types";

export type CalendarViewMode = "agenda" | "week" | "day";

export function Shell({
  body,
  onRefresh,
}: {
  body: ReactNode;
  onRefresh: (() => void) | null;
}) {
  return (
    <section className="admin-panel" aria-labelledby="calendar-title">
      <header className="admin-toolbar">
        <div className="toolbar-title">
          <CalendarDays aria-hidden="true" size={18} />
          <h1 id="calendar-title">Calendar</h1>
        </div>
        {onRefresh ? (
          <button
            className="icon-button"
            type="button"
            title="Refresh"
            aria-label="Refresh calendar"
            onClick={onRefresh}
          >
            <RefreshCw aria-hidden="true" size={16} />
          </button>
        ) : null}
      </header>
      {body}
    </section>
  );
}

export function TextInput({ label, value, onChange }: FieldProps) {
  return (
    <label className="field-control">
      <span>{label}</span>
      <input value={value} onChange={(e) => onChange(e.currentTarget.value)} />
    </label>
  );
}

export function DateInput({ label, value, onChange }: FieldProps) {
  return (
    <label className="field-control">
      <span>{label}</span>
      <input
        type="datetime-local"
        value={value}
        onChange={(e) => onChange(e.currentTarget.value)}
      />
    </label>
  );
}

export function TextArea({ label, value, onChange }: FieldProps) {
  return (
    <label className="field-control">
      <span>{label}</span>
      <textarea
        value={value}
        onChange={(e) => onChange(e.currentTarget.value)}
      />
    </label>
  );
}

export function ContactSelect({
  contacts,
  value,
  onChange,
}: {
  contacts: Contact[];
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="field-control">
      <span>Contact</span>
      <select value={value} onChange={(e) => onChange(e.currentTarget.value)}>
        <option value="">None</option>
        {contacts.map((contact) => (
          <option key={contact.id} value={contact.id}>
            {contact.display_name}
          </option>
        ))}
      </select>
    </label>
  );
}

export function EventSelect({
  events,
  value,
  onChange,
}: {
  events: CalendarEvent[];
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="field-control">
      <span>Event</span>
      <select value={value} onChange={(e) => onChange(e.currentTarget.value)}>
        <option value="">None</option>
        {events.map((event) => (
          <option key={event.id} value={event.id}>
            {event.title}
          </option>
        ))}
      </select>
    </label>
  );
}

export function CalendarViewControls({
  value,
  onChange,
}: {
  value: CalendarViewMode;
  onChange: (value: CalendarViewMode) => void;
}) {
  return (
    <div
      className="calendar-view-controls"
      role="group"
      aria-label="Calendar view"
    >
      {calendarViews.map((view) => (
        <button
          key={view}
          className="secondary-button compact-button"
          type="button"
          aria-pressed={value === view}
          onClick={() => onChange(view)}
        >
          {view}
        </button>
      ))}
    </div>
  );
}

type FieldProps = {
  label: string;
  value: string;
  onChange: (value: string) => void;
};

export function EventList({
  events,
  contactName,
  onStatus,
}: {
  events: CalendarEvent[];
  contactName: (id: string | null) => string;
  onStatus: (id: string, status: CalendarEventStatus) => void;
}) {
  return (
    <section className="business-list">
      <h2>Events</h2>
      {events.map((event) => (
        <article key={event.id}>
          <strong>{event.title}</strong>
          <span>{event.starts_at}</span>
          <small>{contactName(event.contact_id)}</small>
          <StatusSelect
            value={event.status}
            values={eventStatuses}
            onChange={(status) =>
              onStatus(event.id, status as CalendarEventStatus)
            }
          />
        </article>
      ))}
    </section>
  );
}

export function BookingList({
  bookings,
  contactName,
  onStatus,
}: {
  bookings: Booking[];
  contactName: (id: string | null) => string;
  onStatus: (id: string, status: BookingStatus) => void;
}) {
  return (
    <section className="business-list">
      <h2>Bookings</h2>
      {bookings.map((booking) => (
        <article key={booking.id}>
          <strong>{booking.title}</strong>
          <span>{booking.starts_at}</span>
          <small>{contactName(booking.contact_id)}</small>
          <StatusSelect
            value={booking.status}
            values={bookingStatuses}
            onChange={(status) => onStatus(booking.id, status as BookingStatus)}
          />
        </article>
      ))}
    </section>
  );
}

export function IcsCandidateList({
  candidates,
  onUse,
}: {
  candidates: IcsCandidate[];
  onUse: (candidate: IcsCandidate) => void;
}) {
  return (
    <section className="business-list">
      <h2>ICS candidates</h2>
      {candidates.map((candidate) => (
        <article key={candidate.attachment_id}>
          <strong>{candidate.suggested_title ?? candidate.filename}</strong>
          <span>{candidate.suggested_starts_at ?? candidate.subject}</span>
          <small>
            {candidate.suggested_location ??
              candidate.parse_error ??
              candidate.from_address}
          </small>
          <button
            className="secondary-button compact-button"
            type="button"
            onClick={() => onUse(candidate)}
          >
            Use
          </button>
        </article>
      ))}
    </section>
  );
}

function StatusSelect({
  value,
  values,
  onChange,
}: {
  value: string;
  values: string[];
  onChange: (value: string) => void;
}) {
  return (
    <select value={value} onChange={(e) => onChange(e.currentTarget.value)}>
      {values.map((item) => (
        <option key={item} value={item}>
          {item}
        </option>
      ))}
    </select>
  );
}

const eventStatuses = [
  "tentative",
  "confirmed",
  "canceled",
  "completed",
  "missed",
];
const bookingStatuses = [
  "requested",
  "confirmed",
  "canceled",
  "completed",
  "missed",
];
const calendarViews: CalendarViewMode[] = ["agenda", "week", "day"];
