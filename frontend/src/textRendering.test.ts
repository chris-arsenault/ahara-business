import { describe, expect, it } from "vitest";
import { formatBytes, sanitizeAttachmentName } from "./textRendering";

describe("text rendering helpers", () => {
  it("sanitizes path and control characters from attachment names", () => {
    expect(sanitizeAttachmentName("../secret/\u0000invoice.pdf")).toBe(
      "invoice.pdf",
    );
    expect(sanitizeAttachmentName(String.raw`C:\fakepath\payload.exe`)).toBe(
      "payload.exe",
    );
    expect(sanitizeAttachmentName("..\n")).toBe("attachment");
  });

  it("formats attachment byte sizes without inspecting content", () => {
    expect(formatBytes(null)).toBe("unknown size");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(2048)).toBe("2.0 KB");
  });
});
