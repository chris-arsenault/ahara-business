import { useState, type FormEvent } from "react";
import { AlertCircle, LogIn } from "lucide-react";
import { AppMark } from "./appMark";
import type { AuthClient } from "./auth";

export function SignInScreen({
  authClient,
  initialErrorMessage,
  productName,
}: {
  authClient: AuthClient;
  initialErrorMessage: string | null;
  productName: string;
}) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(
    initialErrorMessage,
  );
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
      <AppMark className="auth-mark" size={50} />
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
        {errorMessage ? (
          <p className="auth-error" role="alert">
            <AlertCircle aria-hidden="true" size={16} />
            {errorMessage}
          </p>
        ) : null}
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
