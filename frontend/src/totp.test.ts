import { describe, expect, it } from "vitest";
import { buildTotpSetupUri } from "./totp";

describe("TOTP setup URI", () => {
  it("builds a scanner-compatible otpauth URI from the Cognito setup secret", () => {
    expect(
      buildTotpSetupUri({
        issuer: "Ahara Mail",
        accountName: "chris",
        secretCode: "ABCD EFGH IJKL MNOP",
      }),
    ).toBe(
      "otpauth://totp/Ahara%20Mail:chris?secret=ABCDEFGHIJKLMNOP&issuer=Ahara%20Mail&algorithm=SHA1&digits=6&period=30",
    );
  });
});
