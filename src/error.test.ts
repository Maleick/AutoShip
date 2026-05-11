/**
 * Tests for src/error.ts
 *
 * Run with: node --test dist/error.test.js  (after `npm run build`)
 */

import { describe, it, expect } from "vitest";
import {
  AutoshipError,
  wrapError,
  hookError,
  modelError,
  type ErrorContext,
  type ErrorSuggestion,
} from "./error.js";

describe("AutoshipError", () => {
  it("creates an error with an id, timestamp and stack", () => {
    const err = new AutoshipError("boom");
    expect(err.name).toBe("AutoshipError");
    expect(err.message).toBe("boom");
    expect(err.errorId.startsWith("err-")).toBe(true);
    expect(err.timestamp).toBeTruthy();
    expect(err.stack).toBeTruthy();
    expect(err.context).toEqual({});
    expect(err.suggestions).toEqual([]);
  });

  it("stores context and suggestions", () => {
    const ctx: ErrorContext = { hook: "test", issue: "issue-1" };
    const sug: ErrorSuggestion[] = [{ summary: "restart" }];
    const err = new AutoshipError("fail", ctx, sug);
    expect(err.context).toEqual(ctx);
    expect(err.suggestions).toEqual(sug);
  });

  it("toJSON() includes all fields", () => {
    const err = new AutoshipError("fail", { hook: "h" }, [{ summary: "s" }]);
    const json = err.toJSON();
    expect(json.errorId).toBe(err.errorId);
    expect(json.message).toBe("fail");
    expect(typeof json.stack).toBe("string");
    expect(json.context).toEqual({ hook: "h" });
    expect(json.suggestions).toEqual([{ summary: "s" }]);
  });

  it("toString() includes id, context and suggestions", () => {
    const err = new AutoshipError("fail", { hook: "h" }, [{ summary: "s" }]);
    const str = err.toString();
    expect(str).toContain(err.errorId);
    expect(str).toContain("fail");
    expect(str).toContain("h");
    expect(str).toContain("s");
  });

  it("generates unique error ids", () => {
    const a = new AutoshipError("a");
    const b = new AutoshipError("b");
    expect(a.errorId).not.toBe(b.errorId);
  });
});

describe("wrapError", () => {
  it("wraps a plain string", () => {
    const wrapped = wrapError("oops");
    expect(wrapped).toBeInstanceOf(AutoshipError);
    expect(wrapped.message).toBe("oops");
  });

  it("wraps a native Error preserving stack", () => {
    const native = new Error("native");
    const wrapped = wrapError(native, { hook: "wrap" });
    expect(wrapped.message).toBe("native");
    expect(wrapped.stack).toContain("native");
    expect(wrapped.stack).toContain("--- caused by ---");
    expect(wrapped.context).toEqual({ hook: "wrap" });
  });

  it("returns existing AutoshipError unchanged", () => {
    const original = new AutoshipError("orig", { hook: "a" }, [{ summary: "s" }]);
    const wrapped = wrapError(original);
    expect(wrapped.message).toBe("orig");
    expect(wrapped.context).toEqual({ hook: "a" });
    expect(wrapped.suggestions).toEqual([{ summary: "s" }]);
  });

  it("merges context when wrapping an AutoshipError", () => {
    const original = new AutoshipError("orig", { hook: "a", issue: "i1" });
    const wrapped = wrapError(original, { issue: "i2", category: "cat" });
    expect(wrapped.context).toEqual({ hook: "a", issue: "i2", category: "cat" });
  });

  it("concatenates suggestions when wrapping an AutoshipError", () => {
    const original = new AutoshipError("orig", {}, [{ summary: "s1" }]);
    const wrapped = wrapError(original, {}, [{ summary: "s2" }]);
    expect(wrapped.suggestions).toEqual([{ summary: "s1" }, { summary: "s2" }]);
  });
});

describe("hookError", () => {
  it("builds an error with hook context", () => {
    const err = hookError("install", "missing file", { issue: "issue-42" });
    expect(err.message).toBe("missing file");
    expect(err.context.hook).toBe("install");
    expect(err.context.issue).toBe("issue-42");
  });
});

describe("modelError", () => {
  it("builds an error with model metadata", () => {
    const err = modelError("gpt-4", "timeout", { issue: "issue-7" });
    expect(err.message).toBe("timeout");
    expect(err.context.category).toBe("model_failure");
    expect((err.context.metadata as Record<string, unknown>).model).toBe("gpt-4");
    expect(err.context.issue).toBe("issue-7");
  });
});
