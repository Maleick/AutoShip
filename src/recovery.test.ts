import { describe, it, expect } from "vitest";
import { classifyError, getRecoverySuggestion, formatRecoverySuggestion, DEFAULT_RECOVERY_MAP } from "./recovery.js";

describe("classifyError", () => {
  it("classifies network errors", () => {
    expect(classifyError("connection refused", "ECONNREFUSED")).toBe("network");
    expect(classifyError("network unreachable", "ENETUNREACH")).toBe("network");
    expect(classifyError("DNS lookup failed")).toBe("network");
    expect(classifyError("socket hang up")).toBe("network");
  });

  it("classifies permission errors", () => {
    expect(classifyError("permission denied", "EACCES")).toBe("permission");
    expect(classifyError("unauthorized")).toBe("permission");
    expect(classifyError("access denied")).toBe("permission");
    expect(classifyError("forbidden")).toBe("permission");
  });

  it("classifies timeout errors", () => {
    expect(classifyError("operation timed out", "ETIMEDOUT")).toBe("timeout");
    expect(classifyError("deadline exceeded")).toBe("timeout");
    expect(classifyError("timeout")).toBe("timeout");
  });

  it("classifies unknown errors", () => {
    expect(classifyError("something went wrong")).toBe("unknown");
    expect(classifyError("random error", "EFOO")).toBe("unknown");
  });
});

describe("getRecoverySuggestion", () => {
  it("returns network suggestion", () => {
    const s = getRecoverySuggestion("network");
    expect(s.action).toBe("Reconnect");
    expect(s.command).toBe("opencode-autoship doctor");
  });

  it("returns permission suggestion", () => {
    const s = getRecoverySuggestion("permission");
    expect(s.action).toBe("Check account");
    expect(s.command).toBe("gh auth status");
  });

  it("returns timeout suggestion", () => {
    const s = getRecoverySuggestion("timeout");
    expect(s.action).toBe("Retry");
    expect(s.command).toBe("opencode-autoship doctor");
  });

  it("returns unknown suggestion", () => {
    const s = getRecoverySuggestion("unknown");
    expect(s.action).toBe("Run diagnostics");
    expect(s.command).toBe("opencode-autoship doctor");
  });

  it("falls back to unknown for unmapped kind", () => {
    const custom = { unknown: { suggestion: "fallback" } } as unknown as typeof DEFAULT_RECOVERY_MAP;
    const s = getRecoverySuggestion("network", custom);
    expect(s.suggestion).toBe("fallback");
  });
});

describe("formatRecoverySuggestion", () => {
  it("formats suggestion with action and command", () => {
    const out = formatRecoverySuggestion({
      suggestion: "Try again.",
      action: "Retry",
      command: "npm test",
    });
    expect(out).toContain("💡 Try again.");
    expect(out).toContain("Action: Retry");
    expect(out).toContain("Command: npm test");
  });

  it("formats suggestion without action or command", () => {
    const out = formatRecoverySuggestion({ suggestion: "Just wait." });
    expect(out).toBe("💡 Just wait.");
  });
});
