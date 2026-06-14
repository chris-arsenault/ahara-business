/* eslint-disable max-lines-per-function, react-hooks/exhaustive-deps, react-hooks/set-state-in-effect, react-perf/jsx-no-new-function-as-prop */
import { useEffect, useState, type FormEvent } from "react";
import { CheckCircle2, Plus } from "lucide-react";
import type { ApiClient } from "./api";
import {
  defaultBookingDraft,
  defaultEventDraft,
  fromApiTime,
  toApiTime,
  type BookingDraft,
  type EventDraft,
} from "./calendarBookingDrafts";
import {
  BookingList,
  CalendarViewControls,
  ContactSelect,
  DateInput,
  EventList,
  EventSelect,
  IcsCandidateList,
  Shell,
  TextArea,
  TextInput,
  type CalendarViewMode,
} from "./calendarBookingParts";
import type {
  Booking,
  BookingStatus,
  CalendarEvent,
  CalendarEventStatus,
  Contact,
  CreateBookingRequest,
  CreateCalendarEventRequest,
  IcsCandidate,
} from "./types";

export type CalendarBookingApi = Pick<
  ApiClient,
  | "listCalendarEvents"
  | "createCalendarEvent"
  | "updateCalendarEvent"
  | "listCalendarIcsCandidates"
  | "listBookings"
  | "createBooking"
  | "updateBooking"
  | "listContacts"
>;

type State =
  | { status: "loading" }
  | {
      status: "ready";
      events: CalendarEvent[];
      bookings: Booking[];
      candidates: IcsCandidate[];
      contacts: Contact[];
    }
  | { status: "error"; message: string };

export function CalendarBookingView({
  apiClient,
}: {
  apiClient: CalendarBookingApi;
}) {
  const [state, setState] = useState<State>({ status: "loading" });
  const [eventDraft, setEventDraft] = useState<EventDraft>(() =>
    defaultEventDraft(),
  );
  const [bookingDraft, setBookingDraft] = useState<BookingDraft>(() =>
    defaultBookingDraft(),
  );
  const [view, setView] = useState<CalendarViewMode>("agenda");
  const [actionError, setActionError] = useState<string>();

  async function load() {
    setState({ status: "loading" });
    try {
      const [events, bookings, candidates, contacts] = await Promise.all([
        apiClient.listCalendarEvents(calendarEventQuery(view)),
        apiClient.listBookings(),
        apiClient.listCalendarIcsCandidates(),
        apiClient.listContacts(),
      ]);
      setState({ status: "ready", events, bookings, candidates, contacts });
    } catch (error) {
      setState({
        status: "error",
        message:
          error instanceof Error ? error.message : "Unable to load calendar",
      });
    }
  }

  useEffect(() => {
    void load();
  }, [apiClient, view]);

  if (state.status === "loading") {
    return (
      <Shell
        body={<div className="empty-state">Loading calendar</div>}
        onRefresh={null}
      />
    );
  }
  if (state.status === "error") {
    return (
      <Shell
        body={<div className="error-state">{state.message}</div>}
        onRefresh={null}
      />
    );
  }

  const contactName = (id: string | null) =>
    state.contacts.find((contact) => contact.id === id)?.display_name ?? "";

  return (
    <Shell
      body={
        <>
          {actionError ? (
            <div className="error-state compact-error" role="alert">
              {actionError}
            </div>
          ) : null}
          <CalendarViewControls value={view} onChange={setView} />
          <div className="business-grid">
            <form
              className="business-form"
              onSubmit={(e) => void createEvent(e)}
            >
              <h2>Event</h2>
              <TextInput
                label="Title"
                value={eventDraft.title}
                onChange={patchEvent("title")}
              />
              <DateInput
                label="Starts"
                value={eventDraft.starts_at}
                onChange={patchEvent("starts_at")}
              />
              <DateInput
                label="Ends"
                value={eventDraft.ends_at}
                onChange={patchEvent("ends_at")}
              />
              <TextInput
                label="Timezone"
                value={eventDraft.timezone}
                onChange={patchEvent("timezone")}
              />
              <ContactSelect
                contacts={state.contacts}
                value={eventDraft.contact_id}
                onChange={patchEvent("contact_id")}
              />
              <TextInput
                label="Location"
                value={eventDraft.location}
                onChange={patchEvent("location")}
              />
              <TextArea
                label="Description"
                value={eventDraft.description}
                onChange={patchEvent("description")}
              />
              <button className="secondary-button" type="submit">
                <Plus aria-hidden="true" size={15} />
                Add event
              </button>
            </form>
            <form
              className="business-form"
              onSubmit={(e) => void createBooking(e)}
            >
              <h2>Booking</h2>
              <TextInput
                label="Title"
                value={bookingDraft.title}
                onChange={patchBooking("title")}
              />
              <EventSelect
                events={state.events}
                value={bookingDraft.calendar_event_id}
                onChange={patchBooking("calendar_event_id")}
              />
              <DateInput
                label="Starts"
                value={bookingDraft.starts_at}
                onChange={patchBooking("starts_at")}
              />
              <DateInput
                label="Ends"
                value={bookingDraft.ends_at}
                onChange={patchBooking("ends_at")}
              />
              <ContactSelect
                contacts={state.contacts}
                value={bookingDraft.contact_id}
                onChange={patchBooking("contact_id")}
              />
              <TextInput
                label="Location"
                value={bookingDraft.location}
                onChange={patchBooking("location")}
              />
              <TextArea
                label="Notes"
                value={bookingDraft.notes}
                onChange={patchBooking("notes")}
              />
              <button className="secondary-button" type="submit">
                <CheckCircle2 aria-hidden="true" size={15} />
                Add booking
              </button>
            </form>
          </div>
          <div className="business-grid wide">
            <EventList
              events={state.events}
              contactName={contactName}
              onStatus={updateEventStatus}
            />
            <BookingList
              bookings={state.bookings}
              contactName={contactName}
              onStatus={updateBookingStatus}
            />
            <IcsCandidateList
              candidates={state.candidates}
              onUse={useCandidate}
            />
          </div>
        </>
      }
      onRefresh={() => void load()}
    />
  );

  function patchEvent(field: keyof EventDraft) {
    return (value: string) =>
      setEventDraft((current) => ({ ...current, [field]: value }));
  }

  function patchBooking(field: keyof BookingDraft) {
    return (value: string) =>
      setBookingDraft((current) => ({ ...current, [field]: value }));
  }

  async function createEvent(event: FormEvent) {
    event.preventDefault();
    const request: CreateCalendarEventRequest = {
      title: eventDraft.title,
      starts_at: toApiTime(eventDraft.starts_at),
      ends_at: toApiTime(eventDraft.ends_at),
      timezone: eventDraft.timezone || "UTC",
      location: eventDraft.location || null,
      description: eventDraft.description || null,
      contact_id: eventDraft.contact_id || null,
      source_message_id: eventDraft.source_message_id,
      source_attachment_id: eventDraft.source_attachment_id,
      status: eventDraft.status,
    };
    await runAction(async () => {
      await apiClient.createCalendarEvent(request);
      setEventDraft(defaultEventDraft());
    });
  }

  async function createBooking(event: FormEvent) {
    event.preventDefault();
    const request: CreateBookingRequest = {
      title: bookingDraft.title,
      starts_at: toApiTime(bookingDraft.starts_at),
      ends_at: toApiTime(bookingDraft.ends_at),
      calendar_event_id: bookingDraft.calendar_event_id || null,
      contact_id: bookingDraft.contact_id || null,
      location: bookingDraft.location || null,
      notes: bookingDraft.notes || null,
      status: "requested",
    };
    await runAction(async () => {
      await apiClient.createBooking(request);
      setBookingDraft(defaultBookingDraft());
    });
  }

  async function updateEventStatus(id: string, status: CalendarEventStatus) {
    await runAction(() => apiClient.updateCalendarEvent(id, { status }));
  }

  async function updateBookingStatus(id: string, status: BookingStatus) {
    await runAction(() => apiClient.updateBooking(id, { status }));
  }

  function useCandidate(candidate: IcsCandidate) {
    setEventDraft((current) => ({
      ...current,
      title:
        candidate.suggested_title || candidate.subject || candidate.filename,
      starts_at: candidateTime(
        candidate.suggested_starts_at,
        current.starts_at,
      ),
      ends_at: candidateTime(candidate.suggested_ends_at, current.ends_at),
      timezone: candidate.suggested_timezone || current.timezone,
      status: candidate.suggested_status ?? current.status,
      contact_id: candidate.contact_id ?? "",
      source_message_id: candidate.message_id,
      source_attachment_id: candidate.attachment_id,
      location: candidate.suggested_location ?? current.location,
      description: candidate.suggested_description || candidate.filename,
    }));
  }

  async function runAction(action: () => Promise<unknown>) {
    setActionError(undefined);
    try {
      await action();
      await load();
    } catch (error) {
      setActionError(error instanceof Error ? error.message : "Action failed");
    }
  }
}

function calendarEventQuery(view: CalendarViewMode) {
  const startsFrom = startOfDay(new Date());
  const startsTo = addDays(startsFrom, daysForView(view));
  return {
    starts_from: startsFrom.toISOString(),
    starts_to: startsTo.toISOString(),
    limit: 250,
  };
}

function daysForView(view: CalendarViewMode) {
  if (view === "day") {
    return 1;
  }
  if (view === "week") {
    return 7;
  }
  return 90;
}

function startOfDay(value: Date) {
  return new Date(value.getFullYear(), value.getMonth(), value.getDate());
}

function addDays(value: Date, days: number) {
  return new Date(value.getTime() + days * 24 * 60 * 60 * 1000);
}

function candidateTime(value: string | null, fallback: string) {
  return value ? fromApiTime(value) || fallback : fallback;
}
