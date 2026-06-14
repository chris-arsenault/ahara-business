import type { ReactNode } from "react";
import { AppAuthorizationsView } from "./appAuthorizations";
import {
  isAppAuthorizationsApi,
  type AppAuthorizationsApi,
} from "./appAuthorizationsTypes";
import {
  CalendarBookingView,
  type CalendarBookingApi,
} from "./calendarBooking";
import { ForwardingAuditView } from "./forwardingAudit";
import {
  isForwardingAuditApi,
  type ForwardingAuditApi,
} from "./forwardingAuditTypes";
import { MailboxView, type MailboxApi } from "./mailbox";
import { RoutingAdmin, type RoutingAdminApi } from "./routingAdmin";
import { SharedFilesView, type SharedFilesApi } from "./sharedFiles";
import { isSharedFilesApi } from "./sharedFilesTypes";

export type ActiveView =
  | "mailbox"
  | "contacts"
  | "authorizations"
  | "calendar"
  | "routing"
  | "forwarding"
  | "files";
export type AppApi = MailboxApi &
  Partial<
    RoutingAdminApi &
      SharedFilesApi &
      CalendarBookingApi &
      ForwardingAuditApi &
      AppAuthorizationsApi
  >;

export function WorkspaceView({
  activeView,
  apiClient,
}: {
  activeView: ActiveView;
  apiClient: AppApi;
}) {
  const view = viewFor(activeView, apiClient);
  if (view) {
    return view;
  }
  return (
    <div className="empty-state">
      {activeView === "contacts" ? "Contacts" : "Unavailable"}
    </div>
  );
}

function viewFor(activeView: ActiveView, apiClient: AppApi): ReactNode | null {
  const views: Record<ActiveView, () => ReactNode | null> = {
    authorizations: () =>
      isAppAuthorizationsApi(apiClient) ? (
        <AppAuthorizationsView apiClient={apiClient} />
      ) : null,
    calendar: () =>
      isCalendarBookingApi(apiClient) ? (
        <CalendarBookingView apiClient={apiClient} />
      ) : null,
    contacts: () => null,
    files: () =>
      isSharedFilesApi(apiClient) ? (
        <SharedFilesView apiClient={apiClient} />
      ) : null,
    forwarding: () =>
      isForwardingAuditApi(apiClient) ? (
        <ForwardingAuditView apiClient={apiClient} />
      ) : null,
    mailbox: () => <MailboxView apiClient={apiClient} />,
    routing: () =>
      isRoutingAdminApi(apiClient) ? (
        <RoutingAdmin apiClient={apiClient} />
      ) : null,
  };
  return views[activeView]();
}

function isCalendarBookingApi(
  apiClient: Partial<CalendarBookingApi>,
): apiClient is CalendarBookingApi {
  return Boolean(
    apiClient.listCalendarEvents &&
    apiClient.createCalendarEvent &&
    apiClient.updateCalendarEvent &&
    apiClient.listCalendarIcsCandidates &&
    apiClient.listBookings &&
    apiClient.createBooking &&
    apiClient.updateBooking &&
    apiClient.listContacts,
  );
}

function isRoutingAdminApi(
  apiClient: Partial<RoutingAdminApi>,
): apiClient is RoutingAdminApi {
  return Boolean(
    apiClient.listDomains &&
    apiClient.updateDomain &&
    apiClient.addAddress &&
    apiClient.deactivateAddress &&
    apiClient.listForwardingRules &&
    apiClient.upsertForwardingRule &&
    apiClient.deactivateForwardingRule,
  );
}
