/* eslint-disable complexity, max-lines-per-function, sonarjs/no-nested-conditional, sonarjs/void-use */
import { useEffect, useMemo, useState, type FormEvent } from "react";
import { AlertCircle, LogIn, LogOut, Mail, Route, Users } from "lucide-react";
import { QRCodeSVG } from "qrcode.react";
import { createApiClient } from "./api";
import { createAuthClient, type AuthClient, type AuthState } from "./auth";
import { config } from "./config";
import { MailboxView, type MailboxApi } from "./mailbox";
import { RoutingAdmin, type RoutingAdminApi } from "./routingAdmin";
import { buildTotpSetupUri } from "./totp";

type AppProps = {
  authClient?: AuthClient;
  apiClient?: MailboxApi & Partial<RoutingAdminApi>;
};

export function App({ authClient: injectedAuth, apiClient }: AppProps) {
  const authClient = useMemo(
    () => injectedAuth ?? createAuthClient(),
    [injectedAuth],
  );
  const [authState, setAuthState] = useState<AuthState>(authClient.getState());
  const [activeView, setActiveView] = useState<
    "mailbox" | "contacts" | "routing"
  >("mailbox");
  const appApiClient = useMemo<MailboxApi & Partial<RoutingAdminApi>>(
    () =>
      apiClient ??
      createApiClient({
        getAccessToken: () => authClient.getAccessToken(),
      }),
    [apiClient, authClient],
  );

  useEffect(() => {
    const unsubscribe = authClient.subscribe(setAuthState);
    void authClient.init();
    return unsubscribe;
  }, [authClient]);

  if (authState.status === "loading") {
    return (
      <main className="auth-screen" aria-busy="true">
        <p>{config.productName}</p>
        <h1>Loading</h1>
      </main>
    );
  }

  if (authState.status === "signed-out") {
    return (
      <SignInScreen authClient={authClient} productName={config.productName} />
    );
  }

  if (authState.status === "mfa-required") {
    return (
      <MfaCodeScreen
        authClient={authClient}
        challenge={authState.challenge}
        productName={config.productName}
      />
    );
  }

  if (authState.status === "mfa-setup") {
    return (
      <MfaSetupScreen
        authClient={authClient}
        productName={config.productName}
        secretCode={authState.secretCode}
        username={authState.username}
      />
    );
  }

  if (authState.status === "error") {
    return (
      <main className="auth-screen" role="alert">
        <AlertCircle aria-hidden="true" size={20} />
        <h1>Auth error</h1>
        <p>{authState.message}</p>
      </main>
    );
  }

  return (
    <main className="app-layout">
      <aside className="app-sidebar" aria-label="Application navigation">
        <div className="brand-lockup">
          <Mail aria-hidden="true" size={20} />
          <span>{config.productName}</span>
        </div>
        <nav className="app-nav">
          <button
            className="nav-button"
            data-active={activeView === "mailbox"}
            type="button"
            onClick={() => setActiveView("mailbox")}
          >
            <Mail aria-hidden="true" size={17} />
            Mailbox
          </button>
          <button
            className="nav-button"
            data-active={activeView === "contacts"}
            type="button"
            onClick={() => setActiveView("contacts")}
          >
            <Users aria-hidden="true" size={17} />
            Contacts
          </button>
          <button
            className="nav-button"
            data-active={activeView === "routing"}
            type="button"
            onClick={() => setActiveView("routing")}
          >
            <Route aria-hidden="true" size={17} />
            Routing
          </button>
        </nav>
        <div className="account-strip">
          <span>
            {authState.user.email ?? authState.user.username ?? "Signed in"}
          </span>
          <button
            className="icon-button"
            type="button"
            title="Sign out"
            aria-label="Sign out"
            onClick={() => void authClient.logout()}
          >
            <LogOut aria-hidden="true" size={17} />
          </button>
        </div>
      </aside>
      <section className="workspace">
        {activeView === "mailbox" ? (
          <MailboxView apiClient={appApiClient} />
        ) : activeView === "routing" &&
          appApiClient.listDomains &&
          appApiClient.updateDomain &&
          appApiClient.addAddress &&
          appApiClient.deactivateAddress &&
          appApiClient.listForwardingRules &&
          appApiClient.upsertForwardingRule &&
          appApiClient.deactivateForwardingRule ? (
          <RoutingAdmin apiClient={appApiClient as RoutingAdminApi} />
        ) : (
          <div className="empty-state">
            {activeView === "contacts" ? "Contacts" : "Routing"}
          </div>
        )}
      </section>
    </main>
  );
}

function MfaCodeScreen({
  authClient,
  challenge,
  productName,
}: {
  authClient: AuthClient;
  challenge: "sms" | "totp";
  productName: string;
}) {
  const [code, setCode] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setErrorMessage(null);
    setSubmitting(true);
    try {
      await authClient.confirmMfa(code.trim());
    } catch (error) {
      setErrorMessage(
        error instanceof Error ? error.message : "Unable to verify code",
      );
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <main className="auth-screen">
      <p>{productName}</p>
      <h1>Verify sign in</h1>
      <form className="auth-form" onSubmit={handleSubmit}>
        <label>
          <span>
            {challenge === "sms"
              ? "SMS verification code"
              : "Authenticator code"}
          </span>
          <input
            autoComplete="one-time-code"
            inputMode="numeric"
            name="mfa-code"
            value={code}
            onChange={(event) => setCode(event.target.value)}
          />
        </label>
        {errorMessage ? <p className="auth-error">{errorMessage}</p> : null}
        <button
          className="primary-button"
          disabled={submitting || !code.trim()}
          type="submit"
        >
          <LogIn aria-hidden="true" size={18} />
          Verify
        </button>
      </form>
    </main>
  );
}

function MfaSetupScreen({
  authClient,
  productName,
  secretCode,
  username,
}: {
  authClient: AuthClient;
  productName: string;
  secretCode: string;
  username?: string;
}) {
  const [code, setCode] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const totpUri = useMemo(
    () =>
      buildTotpSetupUri({
        issuer: productName,
        accountName: username ?? productName,
        secretCode,
      }),
    [productName, secretCode, username],
  );

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setErrorMessage(null);
    setSubmitting(true);
    try {
      await authClient.verifyMfaSetup(code.trim());
    } catch (error) {
      setErrorMessage(
        error instanceof Error ? error.message : "Unable to verify code",
      );
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <main className="auth-screen">
      <p>{productName}</p>
      <h1>Set up MFA</h1>
      <form className="auth-form" onSubmit={handleSubmit}>
        <div className="mfa-qr">
          <QRCodeSVG
            aria-label="Authenticator setup QR code"
            className="mfa-qr-code"
            level="M"
            marginSize={4}
            role="img"
            size={180}
            value={totpUri}
          />
        </div>
        <div className="mfa-secret">
          <span>Authenticator setup key</span>
          <code>{secretCode}</code>
        </div>
        <label>
          <span>Authenticator code</span>
          <input
            autoComplete="one-time-code"
            inputMode="numeric"
            name="mfa-setup-code"
            value={code}
            onChange={(event) => setCode(event.target.value)}
          />
        </label>
        {errorMessage ? <p className="auth-error">{errorMessage}</p> : null}
        <button
          className="primary-button"
          disabled={submitting || !code.trim()}
          type="submit"
        >
          <LogIn aria-hidden="true" size={18} />
          Verify
        </button>
      </form>
    </main>
  );
}

function SignInScreen({
  authClient,
  productName,
}: {
  authClient: AuthClient;
  productName: string;
}) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setErrorMessage(null);
    setSubmitting(true);
    try {
      await authClient.signIn(username.trim(), password);
    } catch (error) {
      setErrorMessage(
        error instanceof Error ? error.message : "Authentication failed",
      );
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <main className="auth-screen">
      <p>{productName}</p>
      <h1>Sign in</h1>
      <form className="auth-form" onSubmit={handleSubmit}>
        <label>
          <span>Username</span>
          <input
            autoComplete="username"
            name="username"
            type="text"
            value={username}
            onChange={(event) => setUsername(event.target.value)}
          />
        </label>
        <label>
          <span>Password</span>
          <input
            autoComplete="current-password"
            name="password"
            type="password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
          />
        </label>
        {errorMessage ? <p className="auth-error">{errorMessage}</p> : null}
        <button
          className="primary-button"
          disabled={submitting || !username.trim() || !password}
          type="submit"
        >
          <LogIn aria-hidden="true" size={18} />
          Sign in
        </button>
      </form>
    </main>
  );
}

export default App;
