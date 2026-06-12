import {
  AuthenticationDetails,
  CognitoUser,
  CognitoUserPool,
  type CognitoUserSession,
} from "amazon-cognito-identity-js";
import { config as runtimeConfig } from "./config";

export type AuthState =
  | { status: "loading" }
  | { status: "signed-out" }
  | { status: "mfa-required"; challenge: MfaChallenge }
  | { status: "mfa-setup"; secretCode: string; username?: string }
  | { status: "signed-in"; user: AuthUser }
  | { status: "error"; message: string };

export type MfaChallenge = "sms" | "totp";

export type AuthUser = {
  subject?: string;
  email?: string;
  username?: string;
};

export type AuthClient = {
  getState: () => AuthState;
  subscribe: (listener: (state: AuthState) => void) => () => void;
  init: () => Promise<AuthState>;
  signIn: (username: string, password: string) => Promise<void>;
  confirmMfa: (code: string) => Promise<void>;
  verifyMfaSetup: (code: string) => Promise<void>;
  logout: () => Promise<void>;
  getAccessToken: (request?: AccessTokenRequest) => Promise<string | undefined>;
};

export type AccessTokenRequest = {
  forceRefresh?: boolean;
};

type SessionLike = {
  getAccessToken: () => { getJwtToken: () => string };
  getIdToken: () => { decodePayload: () => Record<string, unknown> };
  isValid: () => boolean;
};

type CognitoAdapter = {
  getSession: () => Promise<SessionLike | null>;
  refreshSession: () => Promise<SessionLike | null>;
  signIn: (username: string, password: string) => Promise<SignInResult>;
  confirmMfa: (code: string) => Promise<SessionLike>;
  verifyMfaSetup: (code: string) => Promise<SessionLike>;
  signOut: () => void;
};

type SignInResult =
  | { status: "signed-in"; session: SessionLike }
  | { status: "mfa-required"; challenge: MfaChallenge }
  | { status: "mfa-setup"; secretCode: string; username?: string };

export type CreateAuthClientOptions = {
  adapter?: CognitoAdapter;
  config?: typeof runtimeConfig;
};

class BrowserAuthClient implements AuthClient {
  private state: AuthState = { status: "loading" };
  private session: SessionLike | null = null;
  private readonly listeners = new Set<(state: AuthState) => void>();
  private readonly adapter: CognitoAdapter;

  constructor(adapter: CognitoAdapter) {
    this.adapter = adapter;
  }

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
    this.setState({ status: "loading" });
    try {
      const session = await this.adapter.getSession();
      this.session = session;
      this.setState(stateFromSession(session));
    } catch (error) {
      this.session = null;
      this.setState({ status: "error", message: authErrorMessage(error) });
    }
    return this.state;
  }

  async signIn(username: string, password: string) {
    try {
      const result = await this.adapter.signIn(username, password);
      this.applySignInResult(result);
    } catch (error) {
      this.session = null;
      this.setState({ status: "signed-out" });
      throw error;
    }
  }

  async confirmMfa(code: string) {
    const session = await this.adapter.confirmMfa(code);
    this.session = session;
    this.setState(stateFromSession(session));
  }

  async verifyMfaSetup(code: string) {
    const session = await this.adapter.verifyMfaSetup(code);
    this.session = session;
    this.setState(stateFromSession(session));
  }

  async logout() {
    this.adapter.signOut();
    this.session = null;
    this.setState({ status: "signed-out" });
  }

  async getAccessToken(request: AccessTokenRequest = {}) {
    if (this.state.status !== "signed-in") {
      return undefined;
    }

    try {
      const session = request.forceRefresh
        ? await this.adapter.refreshSession()
        : await this.adapter.getSession();
      this.session = session;
      const state = stateFromSession(session);
      this.setState(state);
      return state.status === "signed-in"
        ? session?.getAccessToken().getJwtToken()
        : undefined;
    } catch {
      this.session = null;
      this.setState({ status: "signed-out" });
      return undefined;
    }
  }

  private setState(state: AuthState) {
    this.state = state;
    this.listeners.forEach((listener) => listener(state));
  }

  private applySignInResult(result: SignInResult) {
    if (result.status === "signed-in") {
      this.session = result.session;
      this.setState(stateFromSession(result.session));
      return;
    }
    this.session = null;
    this.setState(result);
  }
}

export function createAuthClient(
  options: CreateAuthClientOptions = {},
): AuthClient {
  const cfg = options.config ?? runtimeConfig;
  const adapter = options.adapter ?? createCognitoAdapter(cfg);
  return new BrowserAuthClient(adapter);
}

function createCognitoAdapter(cfg: typeof runtimeConfig): CognitoAdapter {
  const userPool = new CognitoUserPool({
    UserPoolId: requiredConfig(cfg.cognitoUserPoolId, "cognitoUserPoolId"),
    ClientId: requiredConfig(cfg.cognitoClientId, "cognitoClientId"),
  });
  let pendingUser: CognitoUser | null = null;
  let pendingMfaType: "SMS_MFA" | "SOFTWARE_TOKEN_MFA" | null = null;

  return {
    getSession: () => currentSession(userPool),
    refreshSession: () => refreshCurrentSession(userPool),
    signIn: (username: string, password: string) => {
      pendingUser = null;
      pendingMfaType = null;
      return signInWithUserPool(userPool, username, password, {
        setPendingMfaUser: (user, mfaType) => {
          pendingUser = user;
          pendingMfaType = mfaType;
        },
        setPendingSetupUser: (user) => {
          pendingUser = user;
          pendingMfaType = null;
        },
      });
    },
    confirmMfa: (code: string) =>
      confirmPendingMfa(pendingUser, pendingMfaType, code),
    verifyMfaSetup: (code: string) => verifyPendingMfaSetup(pendingUser, code),
    signOut: () => {
      pendingUser?.signOut();
      pendingUser = null;
      pendingMfaType = null;
      userPool.getCurrentUser()?.signOut();
    },
  };
}

function currentSession(
  userPool: CognitoUserPool,
): Promise<CognitoUserSession | null> {
  const user = userPool.getCurrentUser();
  if (!user) {
    return Promise.resolve(null);
  }

  return new Promise((resolve, reject) => {
    user.getSession(
      (error: Error | null, session: CognitoUserSession | null) => {
        if (error) {
          reject(error);
          return;
        }
        resolve(session);
      },
    );
  });
}

function refreshCurrentSession(
  userPool: CognitoUserPool,
): Promise<CognitoUserSession | null> {
  const user = userPool.getCurrentUser();
  if (!user) {
    return Promise.resolve(null);
  }

  return new Promise((resolve, reject) => {
    user.getSession(
      (sessionError: Error | null, session: CognitoUserSession | null) => {
        if (sessionError) {
          reject(sessionError);
          return;
        }
        if (!session) {
          resolve(null);
          return;
        }
        user.refreshSession(
          session.getRefreshToken(),
          (
            refreshError: Error | null,
            refreshedSession: CognitoUserSession | null,
          ) => {
            if (refreshError) {
              reject(refreshError);
              return;
            }
            resolve(refreshedSession);
          },
        );
      },
    );
  });
}

function signInWithUserPool(
  userPool: CognitoUserPool,
  username: string,
  password: string,
  pending: {
    setPendingMfaUser: (
      user: CognitoUser,
      mfaType: "SMS_MFA" | "SOFTWARE_TOKEN_MFA",
    ) => void;
    setPendingSetupUser: (user: CognitoUser) => void;
  },
): Promise<SignInResult> {
  const user = new CognitoUser({ Username: username, Pool: userPool });
  const details = new AuthenticationDetails({
    Username: username,
    Password: password,
  });

  return new Promise((resolve, reject) => {
    user.authenticateUser(details, {
      onSuccess: (session) => resolve({ status: "signed-in", session }),
      onFailure: (error: unknown) => reject(error),
      mfaRequired: () => {
        pending.setPendingMfaUser(user, "SMS_MFA");
        resolve({ status: "mfa-required", challenge: "sms" });
      },
      totpRequired: () => {
        pending.setPendingMfaUser(user, "SOFTWARE_TOKEN_MFA");
        resolve({ status: "mfa-required", challenge: "totp" });
      },
      mfaSetup: () => {
        pending.setPendingSetupUser(user);
        user.associateSoftwareToken({
          associateSecretCode: (secretCode) => {
            resolve({ status: "mfa-setup", secretCode, username });
          },
          onFailure: (error: unknown) => reject(error),
        });
      },
      selectMFAType: () =>
        reject(
          new Error(
            "MFA type selection is required but is not supported by Ahara Mail yet.",
          ),
        ),
      newPasswordRequired: () =>
        reject(
          new Error(
            "A password reset is required before signing in to Ahara Mail.",
          ),
        ),
      customChallenge: () =>
        reject(
          new Error(
            "A custom authentication challenge is required but is not supported by Ahara Mail yet.",
          ),
        ),
    });
  });
}

function confirmPendingMfa(
  pendingUser: CognitoUser | null,
  pendingMfaType: "SMS_MFA" | "SOFTWARE_TOKEN_MFA" | null,
  code: string,
): Promise<CognitoUserSession> {
  if (!pendingUser || !pendingMfaType) {
    return Promise.reject(new Error("No MFA challenge is in progress."));
  }

  return new Promise((resolve, reject) => {
    pendingUser.sendMFACode(
      code,
      {
        onSuccess: (session) => resolve(session),
        onFailure: (error: unknown) => reject(error),
      },
      pendingMfaType,
    );
  });
}

function verifyPendingMfaSetup(
  pendingUser: CognitoUser | null,
  code: string,
): Promise<CognitoUserSession> {
  if (!pendingUser) {
    return Promise.reject(new Error("No MFA setup is in progress."));
  }

  return new Promise((resolve, reject) => {
    pendingUser.verifySoftwareToken(code, "Ahara Mail", {
      onSuccess: (session) => resolve(session),
      onFailure: (error: Error) => reject(error),
    });
  });
}

function stateFromSession(session: SessionLike | null): AuthState {
  if (!session?.isValid()) {
    return { status: "signed-out" };
  }

  const claims = session.getIdToken().decodePayload();
  return {
    status: "signed-in",
    user: {
      subject: stringClaim(claims.sub),
      email: stringClaim(claims.email),
      username: stringClaim(claims["cognito:username"] ?? claims.name),
    },
  };
}

function authErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : "authentication failed";
}

function requiredConfig(value: string, key: string) {
  if (!value) {
    throw new Error(`${key} is required`);
  }
  return value;
}

function stringClaim(value: unknown) {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}
