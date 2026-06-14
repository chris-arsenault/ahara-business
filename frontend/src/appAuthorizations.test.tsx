import "@testing-library/jest-dom/vitest";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AppAuthorizationsView } from "./appAuthorizations";
import type { AppAuthorizationsApi } from "./appAuthorizationsTypes";

afterEach(() => cleanup());

describe("AppAuthorizationsView", () => {
  it("renders users and saves app roles", async () => {
    const calls: string[] = [];
    const user = userEvent.setup();
    render(<AppAuthorizationsView apiClient={api(calls)} />);

    expect(await screen.findByText("chris")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Add" }));
    await user.type(screen.getByLabelText("Username"), "operator");
    await user.type(screen.getByLabelText("Password"), "TemporaryPass123");
    await user.click(screen.getByLabelText("Ahara Business"));
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(calls).toContain("save:operator:ahara-business-app");
  });
});

function api(calls: string[]): AppAuthorizationsApi {
  return {
    listAppAuthorizationUsers: async () => [
      {
        username: "chris",
        email: "chris@example.test",
        display_name: "Chris",
        apps: { "ahara-business-app": "admin" },
      },
    ],
    upsertAppAuthorizationUser: async (username, request) => {
      calls.push(`save:${username}:${Object.keys(request.apps).join(",")}`);
      return {
        username,
        email: request.email ?? null,
        display_name: request.display_name ?? null,
        apps: request.apps,
      };
    },
    deleteAppAuthorizationUser: async (username) => {
      calls.push(`delete:${username}`);
    },
  };
}
