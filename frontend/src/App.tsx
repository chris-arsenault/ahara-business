/* eslint-disable max-lines-per-function, sonarjs/void-use */
import { useEffect, useMemo, useState, type FormEvent } from "react";
import {
  Activity,
  CalendarDays,
  FolderLock,
  Forward,
  LogIn,
  LogOut,
  Mail,
  ReceiptText,
  Route,
  ShieldCheck,
  Users,
  type LucideIcon,
} from "lucide-react";
import { QRCodeSVG } from "qrcode.react";
import { AppMark } from "./appMark";
import { WorkspaceView, type ActiveView, type AppApi } from "./appWorkspace";
import { createApiClient } from "./api";
import { createAuthClient, type AuthClient, type AuthState } from "./auth";
import { config } from "./config";
import { SignInScreen } from "./signInScreen";
import { buildTotpSetupUri } from "./totp";

type AppProps = Partial<{
  authClient: AuthClient;
  apiClient: AppApi;
}>;

type NavItem = {
  icon: LucideIcon;
  label: string;
  view: ActiveView;
};

const navItems: NavItem[] = [
  { icon: Mail, label: "Mailbox", view: "mailbox" },
  { icon: Users, label: "Contacts", view: "contacts" },
  { icon: FolderLock, label: "Files", view: "files" },
  { icon: ShieldCheck, label: "Authorizations", view: "authorizations" },
  { icon: CalendarDays, label: "Calendar", view: "calendar" },
  { icon: ReceiptText, label: "Finance", view: "finance" },
  { icon: Activity, label: "Operations", view: "operations" },
  { icon: Route, label: "Routing", view: "routing" },
  { icon: Forward, label: "Forwarding", view: "forwarding" },
];

export function App({ authClient: injectedAuth, apiClient }: AppProps) {
  const authClient = useMemo(
    () => injectedAuth ?? createAuthClient(),
    [injectedAuth],
  );
  const [authState, setAuthState] = useState<AuthState>(authClient.getState());
  const [activeView, setActiveView] = useState<ActiveView>("mailbox");
  const appApiClient = useMemo<AppApi>(
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
        <AppMark className="auth-mark" size={50} />
        <p>{config.productName}</p>
        <h1>Loading</h1>
      </main>
    );
  }

  if (authState.status === "signed-out") {
    return (
      <SignInScreen
        authClient={authClient}
        initialErrorMessage={null}
        productName={config.productName}
      />
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
      <SignInScreen
        authClient={authClient}
        initialErrorMessage={authState.message}
        productName={config.productName}
      />
    );
  }

  return (
    <main className="app-layout">
      <aside className="app-sidebar" aria-label="Application navigation">
        <div className="brand-lockup">
          <AppMark className="brand-mark" />
          <span>{config.productName}</span>
        </div>
        <nav className="app-nav">
          {navItems.map((item) => (
            <NavButton
              key={item.view}
              active={activeView === item.view}
              item={item}
              onSelect={setActiveView}
            />
          ))}
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
        <WorkspaceView activeView={activeView} apiClient={appApiClient} />
      </section>
    </main>
  );
}

function NavButton({
  active,
  item,
  onSelect,
}: {
  active: boolean;
  item: NavItem;
  onSelect: (view: ActiveView) => void;
}) {
  const Icon = item.icon;
  return (
    <button
      className="nav-button"
      data-active={active}
      type="button"
      onClick={() => onSelect(item.view)}
    >
      <Icon aria-hidden="true" size={17} />
      {item.label}
    </button>
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
      <AppMark className="auth-mark" size={50} />
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
  username: string | null;
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
      <AppMark className="auth-mark" size={50} />
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

export default App;
