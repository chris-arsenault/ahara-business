import "@testing-library/jest-dom/vitest";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { AuthClient } from "./auth";
import { SignInScreen } from "./signInScreen";

afterEach(() => cleanup());

describe("SignInScreen", () => {
  it("offers sign in while displaying a stored-session error", async () => {
    const user = userEvent.setup();
    const signIn = vi.fn(async () => undefined);
    render(
      <SignInScreen
        authClient={fakeAuthClient(signIn)}
        initialErrorMessage="Refresh Token has expired"
        productName="Ahara Mail"
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent(
      "Refresh Token has expired",
    );
    await user.type(screen.getByLabelText("Username"), "chris");
    await user.type(screen.getByLabelText("Password"), "new-session");
    await user.click(screen.getByRole("button", { name: "Sign in" }));

    expect(signIn).toHaveBeenCalledWith("chris", "new-session");
  });
});

function fakeAuthClient(signIn: AuthClient["signIn"]): AuthClient {
  return {
    getState: () => ({ status: "signed-out" }),
    subscribe: () => () => undefined,
    init: async () => ({ status: "signed-out" }),
    signIn,
    confirmMfa: async () => undefined,
    verifyMfaSetup: async () => undefined,
    logout: async () => undefined,
    getAccessToken: async () => undefined,
  };
}
