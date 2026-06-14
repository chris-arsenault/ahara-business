/* eslint-disable max-lines-per-function, react-hooks/exhaustive-deps, react-hooks/set-state-in-effect, react-perf/jsx-no-new-function-as-prop */
import { Save, ShieldCheck, Trash2, UserPlus, X } from "lucide-react";
import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import type { AppAuthorizationsApi } from "./appAuthorizationsTypes";
import type { AppAuthorizationUser } from "./types";

type State =
  | { status: "loading" }
  | { status: "ready"; users: AppAuthorizationUser[] }
  | { status: "error"; message: string };

type Draft = {
  username: string;
  email: string;
  display_name: string;
  password: string;
  apps: Record<string, string>;
  is_new: boolean;
};

const appCatalog = [
  { key: "ahara-business-app", label: "Ahara Business" },
  { key: "svap", label: "SVAP" },
  { key: "canonry", label: "Canonry" },
];
const roles = ["admin", "readonly"];

export function AppAuthorizationsView({
  apiClient,
}: {
  apiClient: AppAuthorizationsApi;
}) {
  const [state, setState] = useState<State>({ status: "loading" });
  const [editing, setEditing] = useState<Draft | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  async function load() {
    setState({ status: "loading" });
    try {
      setState({
        status: "ready",
        users: await apiClient.listAppAuthorizationUsers(),
      });
    } catch (error) {
      setState({
        status: "error",
        message:
          error instanceof Error ? error.message : "Unable to load users",
      });
    }
  }

  useEffect(() => {
    void load();
  }, [apiClient]);

  if (state.status === "loading") {
    return <Shell body={<div className="empty-state">Loading users</div>} />;
  }
  if (state.status === "error") {
    return <Shell body={<div className="error-state">{state.message}</div>} />;
  }

  return (
    <Shell
      body={
        <>
          {actionError ? (
            <div className="error-state compact-error" role="alert">
              {actionError}
            </div>
          ) : null}
          <div className="business-grid">
            <section className="business-list">
              <div className="business-list-heading">
                <h2>Users</h2>
                <button
                  className="secondary-button compact-button"
                  type="button"
                  onClick={() => setEditing(blankDraft())}
                >
                  <UserPlus aria-hidden="true" size={15} />
                  Add
                </button>
              </div>
              {state.users.map((user) => (
                <article key={user.username}>
                  <strong>{user.username}</strong>
                  <span>{user.display_name || ""}</span>
                  <small>{appSummary(user)}</small>
                  <div className="inline-actions">
                    <button
                      className="secondary-button compact-button"
                      type="button"
                      onClick={() => setEditing(draftFromUser(user))}
                    >
                      Edit
                    </button>
                    <button
                      className="icon-button"
                      type="button"
                      title="Delete"
                      aria-label={`Delete ${user.username}`}
                      disabled={user.username === "chris"}
                      onClick={() => void deleteUser(user.username)}
                    >
                      <Trash2 aria-hidden="true" size={15} />
                    </button>
                  </div>
                </article>
              ))}
            </section>
            {editing ? (
              <AuthorizationEditor
                draft={editing}
                setDraft={setEditing}
                onCancel={() => setEditing(null)}
                onSubmit={saveUser}
              />
            ) : (
              <section className="business-list">
                <h2>Applications</h2>
                {appCatalog.map((app) => (
                  <article key={app.key}>
                    <strong>{app.label}</strong>
                    <span>{app.key}</span>
                  </article>
                ))}
              </section>
            )}
          </div>
        </>
      }
    />
  );

  async function saveUser(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!editing) {
      return;
    }
    await runAction(async () => {
      await apiClient.upsertAppAuthorizationUser(editing.username, {
        email: editing.email || null,
        display_name: editing.display_name || null,
        password: editing.password || null,
        apps: editing.apps,
      });
      setEditing(null);
    });
  }

  async function deleteUser(username: string) {
    await runAction(() => apiClient.deleteAppAuthorizationUser(username));
  }

  async function runAction(action: () => Promise<unknown>) {
    setActionError(null);
    try {
      await action();
      await load();
    } catch (error) {
      setActionError(error instanceof Error ? error.message : "Action failed");
    }
  }
}

function Shell({ body }: { body: ReactNode }) {
  return (
    <section className="admin-panel" aria-labelledby="authorizations-title">
      <header className="admin-toolbar">
        <div className="toolbar-title">
          <ShieldCheck aria-hidden="true" size={18} />
          <h1 id="authorizations-title">Authorizations</h1>
        </div>
      </header>
      {body}
    </section>
  );
}

function AuthorizationEditor({
  draft,
  setDraft,
  onCancel,
  onSubmit,
}: {
  draft: Draft;
  setDraft: (draft: Draft) => void;
  onCancel: () => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  return (
    <form className="business-form" onSubmit={onSubmit}>
      <h2>{draft.is_new ? "Add user" : "Edit user"}</h2>
      <label className="field-control">
        <span>Username</span>
        <input
          required
          disabled={!draft.is_new}
          value={draft.username}
          onChange={(event) => patch("username", event.currentTarget.value)}
        />
      </label>
      <label className="field-control">
        <span>Email</span>
        <input
          value={draft.email}
          onChange={(event) => patch("email", event.currentTarget.value)}
        />
      </label>
      <label className="field-control">
        <span>Display name</span>
        <input
          value={draft.display_name}
          onChange={(event) => patch("display_name", event.currentTarget.value)}
        />
      </label>
      {draft.is_new ? (
        <label className="field-control">
          <span>Password</span>
          <input
            required
            minLength={8}
            type="password"
            value={draft.password}
            onChange={(event) => patch("password", event.currentTarget.value)}
          />
        </label>
      ) : null}
      <fieldset className="business-fieldset">
        <legend>App access</legend>
        {appCatalog.map((app) => (
          <div key={app.key} className="app-auth-row">
            <label>
              <input
                type="checkbox"
                checked={Boolean(draft.apps[app.key])}
                onChange={() => toggleApp(app.key)}
              />
              {app.label}
            </label>
            {draft.apps[app.key] ? (
              <select
                value={draft.apps[app.key]}
                onChange={(event) =>
                  setRole(app.key, event.currentTarget.value)
                }
              >
                {roles.map((role) => (
                  <option key={role} value={role}>
                    {role}
                  </option>
                ))}
              </select>
            ) : null}
          </div>
        ))}
      </fieldset>
      <div className="editor-actions">
        <button className="secondary-button" type="submit">
          <Save aria-hidden="true" size={15} />
          Save
        </button>
        <button className="secondary-button" type="button" onClick={onCancel}>
          <X aria-hidden="true" size={15} />
          Cancel
        </button>
      </div>
    </form>
  );

  function patch(field: keyof Draft, value: string) {
    setDraft({ ...draft, [field]: value });
  }

  function toggleApp(key: string) {
    const apps = { ...draft.apps };
    if (apps[key]) {
      delete apps[key];
    } else {
      apps[key] = "admin";
    }
    setDraft({ ...draft, apps });
  }

  function setRole(key: string, role: string) {
    setDraft({ ...draft, apps: { ...draft.apps, [key]: role } });
  }
}

function blankDraft(): Draft {
  return {
    username: "",
    email: "",
    display_name: "",
    password: "",
    apps: {},
    is_new: true,
  };
}

function draftFromUser(user: AppAuthorizationUser): Draft {
  return {
    username: user.username,
    email: user.email ?? "",
    display_name: user.display_name ?? "",
    password: "",
    apps: user.apps,
    is_new: false,
  };
}

function appSummary(user: AppAuthorizationUser) {
  if (user.username === "chris") {
    return "global override";
  }
  return (
    Object.entries(user.apps)
      .map(([key, role]) => `${key}:${role}`)
      .join(", ") || "none"
  );
}
