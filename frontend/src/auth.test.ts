/* eslint-disable max-lines-per-function */
import { describe, expect, it } from "vitest";
import { createAuthClient, type AuthState } from "./auth";

type FakeSession = {
  accessToken: string;
  idClaims: Record<string, unknown>;
} & Partial<{
  valid: boolean;
}>;

class FakeAdapter {
  session: FakeSession | null = null;
  signedOut = false;
  signInUsername?: string;
  signInPassword?: string;
  getSessionError?: Error;
  signInError?: Error;
  signInResult: "signed-in" | "mfa-required" | "mfa-setup" = "signed-in";
  mfaCode?: string;
  setupCode?: string;
  refreshCount = 0;

  async getSession() {
    if (this.getSessionError) {
      throw this.getSessionError;
    }
    return this.session ? sessionLike(this.session) : null;
  }

  async refreshSession() {
    this.refreshCount += 1;
    return this.getSession();
  }

  async signIn(username: string, password: string) {
    this.signInUsername = username;
    this.signInPassword = password;
    if (this.signInError) {
      throw this.signInError;
    }
    if (this.signInResult === "mfa-required") {
      return { status: "mfa-required" as const, challenge: "totp" as const };
    }
    if (this.signInResult === "mfa-setup") {
      return {
        status: "mfa-setup" as const,
        secretCode: "setup-secret",
        username,
      };
    }
    this.session = {
      accessToken: "signed-in-access-token",
      idClaims: {
        sub: "signed-in-sub",
        "cognito:username": username,
      },
    };
    return { status: "signed-in" as const, session: sessionLike(this.session) };
  }

  async confirmMfa(code: string) {
    this.mfaCode = code;
    this.session = {
      accessToken: "mfa-access-token",
      idClaims: { sub: "mfa-sub", "cognito:username": "chris" },
    };
    return sessionLike(this.session);
  }

  async verifyMfaSetup(code: string) {
    this.setupCode = code;
    this.session = {
      accessToken: "setup-access-token",
      idClaims: { sub: "setup-sub", "cognito:username": "chris" },
    };
    return sessionLike(this.session);
  }

  signOut() {
    this.signedOut = true;
    this.session = null;
  }
}

function sessionLike(session: FakeSession) {
  return {
    getAccessToken: () => ({
      getJwtToken: () => session.accessToken,
    }),
    getIdToken: () => ({
      decodePayload: () => session.idClaims,
    }),
    isValid: () => session.valid ?? true,
  };
}

describe("auth client", () => {
  it("starts in loading state", () => {
    const client = createAuthClient({ adapter: new FakeAdapter() });

    expect(client.getState()).toEqual({ status: "loading" });
  });

  it("resolves signed-out state without a stored user", async () => {
    const client = createAuthClient({ adapter: new FakeAdapter() });

    await client.init();

    expect(client.getState()).toEqual({ status: "signed-out" });
    await expect(client.getAccessToken()).resolves.toBeUndefined();
  });

  it("resolves signed-in state and exposes the access token", async () => {
    const adapter = new FakeAdapter();
    adapter.session = {
      accessToken: "access-token",
      idClaims: {
        sub: "user-sub",
        email: "chris@example.test",
        "cognito:username": "chris",
      },
    };
    const client = createAuthClient({ adapter });

    await client.init();

    expect(client.getState()).toEqual<AuthState>({
      status: "signed-in",
      user: {
        subject: "user-sub",
        email: "chris@example.test",
        username: "chris",
      },
    });
    await expect(client.getAccessToken()).resolves.toBe("access-token");
  });

  it("treats invalid stored sessions as signed out", async () => {
    const adapter = new FakeAdapter();
    adapter.session = {
      accessToken: "expired-token",
      idClaims: { sub: "user-sub" },
      valid: false,
    };
    const client = createAuthClient({ adapter });

    await client.init();

    expect(client.getState()).toEqual({ status: "signed-out" });
    await expect(client.getAccessToken()).resolves.toBeUndefined();
  });

  it("signs in with username and password through the Cognito adapter", async () => {
    const adapter = new FakeAdapter();
    const client = createAuthClient({ adapter });

    await client.init();
    await client.signIn("chris", "password");

    expect(adapter.signInUsername).toBe("chris");
    expect(adapter.signInPassword).toBe("password");
    expect(client.getState()).toEqual<AuthState>({
      status: "signed-in",
      user: {
        subject: "signed-in-sub",
        email: null,
        username: "chris",
      },
    });
    await expect(client.getAccessToken()).resolves.toBe(
      "signed-in-access-token",
    );
  });

  it("enters MFA challenge state when Cognito requires a code", async () => {
    const adapter = new FakeAdapter();
    adapter.signInResult = "mfa-required";
    const client = createAuthClient({ adapter });

    await client.init();
    await client.signIn("chris", "password");

    expect(client.getState()).toEqual({
      status: "mfa-required",
      challenge: "totp",
    });
    await expect(client.getAccessToken()).resolves.toBeUndefined();
  });

  it("confirms MFA codes and exposes the resulting access token", async () => {
    const adapter = new FakeAdapter();
    adapter.signInResult = "mfa-required";
    const client = createAuthClient({ adapter });

    await client.init();
    await client.signIn("chris", "password");
    await client.confirmMfa("123456");

    expect(adapter.mfaCode).toBe("123456");
    expect(client.getState()).toEqual<AuthState>({
      status: "signed-in",
      user: { subject: "mfa-sub", email: null, username: "chris" },
    });
    await expect(client.getAccessToken()).resolves.toBe("mfa-access-token");
  });

  it("enters MFA setup state when Cognito requires enrollment", async () => {
    const adapter = new FakeAdapter();
    adapter.signInResult = "mfa-setup";
    const client = createAuthClient({ adapter });

    await client.init();
    await client.signIn("chris", "password");

    expect(client.getState()).toEqual({
      status: "mfa-setup",
      secretCode: "setup-secret",
      username: "chris",
    });
  });

  it("verifies MFA setup codes and exposes the resulting access token", async () => {
    const adapter = new FakeAdapter();
    adapter.signInResult = "mfa-setup";
    const client = createAuthClient({ adapter });

    await client.init();
    await client.signIn("chris", "password");
    await client.verifyMfaSetup("654321");

    expect(adapter.setupCode).toBe("654321");
    expect(client.getState()).toEqual<AuthState>({
      status: "signed-in",
      user: { subject: "setup-sub", email: null, username: "chris" },
    });
    await expect(client.getAccessToken()).resolves.toBe("setup-access-token");
  });

  it("keeps failed sign-in attempts signed out", async () => {
    const adapter = new FakeAdapter();
    adapter.signInError = new Error("Incorrect username or password.");
    const client = createAuthClient({ adapter });

    await client.init();
    await expect(client.signIn("chris", "bad")).rejects.toThrow(
      "Incorrect username or password.",
    );

    expect(client.getState()).toEqual({ status: "signed-out" });
    await expect(client.getAccessToken()).resolves.toBeUndefined();
  });

  it("clears local state on logout", async () => {
    const adapter = new FakeAdapter();
    adapter.session = {
      accessToken: "access-token",
      idClaims: { sub: "user-sub" },
    };
    const client = createAuthClient({ adapter });

    await client.init();
    await client.logout();

    expect(adapter.signedOut).toBe(true);
    expect(client.getState()).toEqual({ status: "signed-out" });
    await expect(client.getAccessToken()).resolves.toBeUndefined();
  });

  it("surfaces stored-session errors as renderable state", async () => {
    const adapter = new FakeAdapter();
    adapter.getSessionError = new Error("session failed");
    const client = createAuthClient({ adapter });

    await client.init();

    expect(client.getState()).toEqual({
      status: "error",
      message: "session failed",
    });
  });

  it("can force-refresh the stored Cognito session before returning an access token", async () => {
    const adapter = new FakeAdapter();
    adapter.session = {
      accessToken: "old-access-token",
      idClaims: { sub: "user-sub", "cognito:username": "chris" },
    };
    const client = createAuthClient({ adapter });

    await client.init();
    adapter.session = {
      accessToken: "refreshed-access-token",
      idClaims: { sub: "user-sub", "cognito:username": "chris" },
    };

    await expect(client.getAccessToken({ forceRefresh: true })).resolves.toBe(
      "refreshed-access-token",
    );
    expect(adapter.refreshCount).toBe(1);
    expect(client.getState()).toEqual<AuthState>({
      status: "signed-in",
      user: { subject: "user-sub", email: null, username: "chris" },
    });
  });
});
