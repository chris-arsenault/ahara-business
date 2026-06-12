/* eslint-disable max-lines-per-function */
import "@testing-library/jest-dom/vitest";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App } from "./App";
import type { AuthClient, AuthState } from "./auth";
import type { MailboxApi } from "./mailbox";
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
      user: { username },
    });
  }

  async confirmMfa(code: string) {
    this.setState({
      status: "signed-in",
      user: { username: `mfa-${code}` },
    });
  }

  async verifyMfaSetup(code: string) {
    this.setState({
      status: "signed-in",
      user: { username: `setup-${code}` },
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

afterEach(() => cleanup());

describe("App", () => {
  it("renders auth loading state", () => {
    render(
      <App
        authClient={new FakeAuthClient({ status: "loading" }, "pending")}
        apiClient={{ fetchMailboxMessages: async () => [message] }}
      />,
    );

    expect(screen.getByText("Loading")).toBeInTheDocument();
    expect(screen.queryByText("Invoice")).not.toBeInTheDocument();
  });

  it("renders signed-out sign-in form", async () => {
    render(
      <App
        authClient={new FakeAuthClient({ status: "signed-out" })}
        apiClient={{ fetchMailboxMessages: async () => [message] }}
      />,
    );

    expect(
      await screen.findByRole("button", { name: "Sign in" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Username")).toBeInTheDocument();
    expect(screen.getByLabelText("Password")).toBeInTheDocument();
  });

  it("submits non-email username and password from the sign-in form", async () => {
    const user = userEvent.setup();
    const authClient = new FakeAuthClient({ status: "signed-out" });
    render(
      <App
        authClient={authClient}
        apiClient={{ fetchMailboxMessages: async () => [message] }}
      />,
    );

    await user.type(screen.getByLabelText("Username"), "chris");
    await user.type(screen.getByLabelText("Password"), "correct-password");
    await user.click(screen.getByRole("button", { name: "Sign in" }));

    expect(authClient.signInUsername).toBe("chris");
    expect(authClient.signInPassword).toBe("correct-password");
  });

  it("renders and submits an MFA verification code", async () => {
    const user = userEvent.setup();
    render(
      <App
        authClient={
          new FakeAuthClient({
            status: "mfa-required",
            challenge: "totp",
          })
        }
        apiClient={{ fetchMailboxMessages: async () => [message] }}
      />,
    );

    await user.type(screen.getByLabelText("Authenticator code"), "123456");
    await user.click(screen.getByRole("button", { name: "Verify" }));

    expect(await screen.findByText("mfa-123456")).toBeInTheDocument();
  });

  it("renders and submits an MFA setup code", async () => {
    const user = userEvent.setup();
    render(
      <App
        authClient={
          new FakeAuthClient({
            status: "mfa-setup",
            secretCode: "setup-secret",
            username: "chris",
          })
        }
        apiClient={{ fetchMailboxMessages: async () => [message] }}
      />,
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
    render(
      <App
        authClient={
          new FakeAuthClient({
            status: "signed-in",
            user: { email: "chris@example.test" },
          })
        }
        apiClient={{ fetchMailboxMessages: async () => [message] }}
      />,
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

    render(
      <App
        authClient={new FakeAuthClient({ status: "loading" }, "pending")}
        apiClient={apiClient}
      />,
    );

    expect(fetched).toBe(false);
    expect(screen.queryByText("Invoice")).not.toBeInTheDocument();
  });

  it("opens the routing admin panel from signed-in navigation", async () => {
    const user = userEvent.setup();
    render(
      <App
        authClient={
          new FakeAuthClient({
            status: "signed-in",
            user: { email: "chris@example.test" },
          })
        }
        apiClient={{
          fetchMailboxMessages: async () => [message],
          listDomains: async () => [
            {
              domain_name: "ahara.io",
              routing_policy: "allowlist",
              active: true,
              addresses: [{ local_part: "chris", active: true }],
            },
          ],
          updateDomain: async (_domainName, request) => ({
            domain_name: "ahara.io",
            routing_policy: request.routing_policy ?? "allowlist",
            active: request.active ?? true,
            addresses: [{ local_part: "chris", active: true }],
          }),
          addAddress: async (_domainName, localPart) => ({
            local_part: localPart,
            active: true,
          }),
          deactivateAddress: async (_domainName, localPart) => ({
            local_part: localPart,
            active: false,
          }),
          listForwardingRules: async () => [],
          upsertForwardingRule: async (request) => ({
            id: "rule-1",
            domain_name: request.domain_name,
            local_part: request.local_part,
            address_id: `${request.domain_name}:${request.local_part}`,
            target_address: request.target_address,
            target_address_normalized: request.target_address.toLowerCase(),
            active: true,
            created_at: null,
            updated_at: null,
          }),
          deactivateForwardingRule: async (ruleId) => ({
            id: ruleId,
            domain_name: "ahara.io",
            local_part: "chris",
            address_id: "ahara.io:chris",
            target_address: "target@example.com",
            target_address_normalized: "target@example.com",
            active: false,
            created_at: null,
            updated_at: null,
          }),
        }}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Routing" }));

    expect(await screen.findByText("ahara.io")).toBeInTheDocument();
  });
});
