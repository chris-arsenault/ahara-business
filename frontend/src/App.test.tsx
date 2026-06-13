/* eslint-disable max-lines-per-function */
import "@testing-library/jest-dom/vitest";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App } from "./App";
import type { AuthClient, AuthState } from "./auth";
import type { MailboxApi } from "./mailbox";
import type { RoutingAdminApi } from "./routingAdmin";
import type { SharedFilesApi } from "./sharedFiles";
import type { MailboxMessageSummary } from "./types";

class FakeAuthClient implements AuthClient {
  private listeners = new Set<(state: AuthState) => void>();
  signInUsername?: string;
  signInPassword?: string;
  signInError?: Error;

  constructor(
    private state: AuthState,
    private readonly initState?: AuthState | "pending",
  ) {}

  getState() {
    return this.state;
  }

  subscribe(listener: (state: AuthState) => void) {
    this.listeners.add(listener);
    listener(this.state);
    return () => {
      this.listeners.delete(listener);
    };
  }

  async init() {
    if (this.initState === "pending") {
      return new Promise<AuthState>(() => undefined);
    }
    if (this.initState) {
      this.setState(this.initState);
    }
    return this.state;
  }

  async signIn(username: string, password: string) {
    this.signInUsername = username;
    this.signInPassword = password;
    if (this.signInError) {
      throw this.signInError;
    }
    this.setState({
      status: "signed-in",
      user: { subject: null, email: null, username },
    });
  }

  async confirmMfa(code: string) {
    this.setState({
      status: "signed-in",
      user: { subject: null, email: null, username: `mfa-${code}` },
    });
  }

  async verifyMfaSetup(code: string) {
    this.setState({
      status: "signed-in",
      user: { subject: null, email: null, username: `setup-${code}` },
    });
  }

  async logout() {
    this.setState({ status: "signed-out" });
  }

  async getAccessToken() {
    return this.state.status === "signed-in" ? "access-token" : undefined;
  }

  private setState(state: AuthState) {
    this.state = state;
    this.listeners.forEach((listener) => listener(state));
  }
}

const message: MailboxMessageSummary = {
  id: "message-1",
  thread_id: "thread-1",
  from_address: "sender@example.test",
  from_display_name: "Sender Display",
  subject: "Invoice",
  snippet: "Plaintext invoice body",
  received_at: "2026-01-01 00:00:00+00",
  is_read: false,
  has_attachments: true,
  attachment_count: 1,
  contact_id: null,
  auth_verdict: "pass",
  spam_result: "pass",
  virus_result: "pass",
  security_disposition: "accepted",
};

const defaultApiClient: MailboxApi = {
  fetchMailboxMessages: async () => [message],
};

function renderApp(
  authClient: AuthClient,
  apiClient: MailboxApi &
    Partial<RoutingAdminApi & SharedFilesApi> = defaultApiClient,
) {
  return render(<App authClient={authClient} apiClient={apiClient} />);
}

afterEach(() => cleanup());

describe("App", () => {
  it("renders auth loading state", () => {
    renderApp(new FakeAuthClient({ status: "loading" }, "pending"));

    expect(screen.getByText("Loading")).toBeInTheDocument();
    expect(screen.queryByText("Invoice")).not.toBeInTheDocument();
  });

  it("renders signed-out sign-in form", async () => {
    renderApp(new FakeAuthClient({ status: "signed-out" }));

    expect(
      await screen.findByRole("button", { name: "Sign in" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Username")).toBeInTheDocument();
    expect(screen.getByLabelText("Password")).toBeInTheDocument();
  });

  it("submits non-email username and password from the sign-in form", async () => {
    const user = userEvent.setup();
    const authClient = new FakeAuthClient({ status: "signed-out" });
    renderApp(authClient);

    await user.type(screen.getByLabelText("Username"), "chris");
    await user.type(screen.getByLabelText("Password"), "correct-password");
    await user.click(screen.getByRole("button", { name: "Sign in" }));

    expect(authClient.signInUsername).toBe("chris");
    expect(authClient.signInPassword).toBe("correct-password");
  });

  it("renders and submits an MFA verification code", async () => {
    const user = userEvent.setup();
    renderApp(
      new FakeAuthClient({
        status: "mfa-required",
        challenge: "totp",
      }),
    );

    await user.type(screen.getByLabelText("Authenticator code"), "123456");
    await user.click(screen.getByRole("button", { name: "Verify" }));

    expect(await screen.findByText("mfa-123456")).toBeInTheDocument();
  });

  it("renders and submits an MFA setup code", async () => {
    const user = userEvent.setup();
    renderApp(
      new FakeAuthClient({
        status: "mfa-setup",
        secretCode: "setup-secret",
        username: "chris",
      }),
    );

    expect(screen.getByText("setup-secret")).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "Authenticator setup QR code" }),
    ).toBeInTheDocument();
    await user.type(screen.getByLabelText("Authenticator code"), "654321");
    await user.click(screen.getByRole("button", { name: "Verify" }));

    expect(await screen.findByText("setup-654321")).toBeInTheDocument();
  });

  it("renders signed-in mailbox list as the first screen", async () => {
    renderApp(
      new FakeAuthClient({
        status: "signed-in",
        user: {
          subject: null,
          email: "chris@example.test",
          username: null,
        },
      }),
    );

    expect(await screen.findByText("Invoice")).toBeInTheDocument();
    expect(screen.getByText("sender@example.test")).toBeInTheDocument();
    expect(screen.getByText("pass")).toBeInTheDocument();
    expect(screen.getByLabelText("Unread message")).toBeInTheDocument();
  });

  it("does not fetch mailbox content while auth is unresolved", () => {
    let fetched = false;
    const apiClient: MailboxApi = {
      fetchMailboxMessages: async () => {
        fetched = true;
        return [message];
      },
    };

    renderApp(new FakeAuthClient({ status: "loading" }, "pending"), apiClient);

    expect(fetched).toBe(false);
    expect(screen.queryByText("Invoice")).not.toBeInTheDocument();
  });

  it("opens the routing admin panel from signed-in navigation", async () => {
    const user = userEvent.setup();
    const apiClient: MailboxApi & Partial<RoutingAdminApi> = {
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
      updateDomain: async (_domainName, request) => ({
        domain_name: "ahara.io",
        routing_policy: request.routing_policy ?? "allowlist",
        active: request.active ?? true,
        raw_retention_days: request.raw_retention_days ?? 365,
        addresses: [
          { local_part: "chris", active: true, raw_retention_days: null },
        ],
      }),
      addAddress: async (_domainName, localPart) => ({
        local_part: localPart,
        active: true,
        raw_retention_days: null,
      }),
      updateAddress: async (_domainName, localPart, request) => ({
        local_part: localPart,
        active: request.active ?? true,
        raw_retention_days: request.raw_retention_days ?? null,
      }),
      deactivateAddress: async (_domainName, localPart) => ({
        local_part: localPart,
        active: false,
        raw_retention_days: null,
      }),
      listForwardingRules: async () => [],
      upsertForwardingRule: async (request) => ({
        id: "rule-1",
        rule_kind: request.local_part ? "address" : "domain",
        domain_name: request.domain_name,
        local_part: request.local_part ?? null,
        address_id: request.local_part
          ? `${request.domain_name}:${request.local_part}`
          : null,
        target_address: request.target_address,
        target_address_normalized: request.target_address.toLowerCase(),
        sender_address_normalized:
          request.sender_address?.toLowerCase() ?? null,
        plus_tag: request.plus_tag?.toLowerCase() ?? null,
        require_auth_pass: request.require_auth_pass ?? true,
        active: true,
        created_at: null,
        updated_at: null,
      }),
      deactivateForwardingRule: async (ruleId) => ({
        id: ruleId,
        rule_kind: "address",
        domain_name: "ahara.io",
        local_part: "chris",
        address_id: "ahara.io:chris",
        target_address: "target@example.com",
        target_address_normalized: "target@example.com",
        sender_address_normalized: null,
        plus_tag: null,
        require_auth_pass: true,
        active: false,
        created_at: null,
        updated_at: null,
      }),
    };
    renderApp(
      new FakeAuthClient({
        status: "signed-in",
        user: {
          subject: null,
          email: "chris@example.test",
          username: null,
        },
      }),
      apiClient,
    );

    await user.click(screen.getByRole("button", { name: "Routing" }));

    expect(await screen.findByText("ahara.io")).toBeInTheDocument();
  });

  it("opens the shared files panel from signed-in navigation", async () => {
    const user = userEvent.setup();
    renderApp(
      new FakeAuthClient({
        status: "signed-in",
        user: {
          subject: null,
          email: "chris@example.test",
          username: null,
        },
      }),
      sharedFilesApi(),
    );

    await user.click(screen.getByRole("button", { name: "Files" }));

    expect(await screen.findByText("Shared files")).toBeInTheDocument();
    expect(screen.getAllByText("master.wav").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Mastering engineer").length).toBeGreaterThan(0);
  });
});

function sharedFilesApi(): MailboxApi & SharedFilesApi {
  return {
    fetchMailboxMessages: async () => [message],
    listAccessPrincipals: async () => [accessPrincipal],
    createAccessPrincipal: async () => accessPrincipal,
    listAccessAudiences: async () => [accessAudience],
    createAccessAudience: async () => accessAudience,
    listAccessAudienceMembers: async () => [accessMember],
    addAccessAudienceMember: async () => accessMember,
    listAccessAssets: async () => [accessAsset],
    uploadAccessAsset: async () => accessAsset,
    listAccessGrants: async () => [accessGrant],
    createAccessGrant: async () => accessGrant,
    revokeAccessGrant: async () => ({ ...accessGrant, revoked_at: "now" }),
  };
}

const accessPrincipal = {
  id: "principal-1",
  principal_kind: "external" as const,
  cognito_sub: null,
  username: "engineer",
  email: null,
  display_name: "Mastering engineer",
  active: true,
};

const accessAudience = {
  id: "audience-1",
  audience_key: "mastering",
  display_name: "Mastering",
  description: null,
  active: true,
};

const accessMember = {
  audience_id: "audience-1",
  principal_id: "principal-1",
};

const accessAsset = {
  id: "asset-1",
  owner_app: "tsonu_music",
  resource_id: null,
  storage_kind: "managed_s3" as const,
  filename: "master.wav",
  content_type: "audio/wav",
  size_bytes: 4096,
  sha256: null,
  active: true,
};

const accessGrant = {
  id: "grant-1",
  principal_id: "principal-1",
  audience_id: null,
  resource_id: null,
  asset_id: "asset-1",
  permission_level: "download" as const,
  expires_at: null,
  revoked_at: null,
};
