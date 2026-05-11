/**
 * Tests for src/error.ts
 *
 * Run with: node --test dist/error.test.js  (after `npm run build`)
 */
import { describe, it } from "node:test";
import * as assert from "node:assert";
import { AutoshipError, wrapError, hookError, modelError, } from "./error.js";
describe("AutoshipError", () => {
    it("creates an error with an id, timestamp and stack", () => {
        const err = new AutoshipError("boom");
        assert.strictEqual(err.name, "AutoshipError");
        assert.strictEqual(err.message, "boom");
        assert.ok(err.errorId.startsWith("err-"), "errorId should start with err-");
        assert.ok(err.timestamp, "timestamp should be set");
        assert.ok(err.stack, "stack should be present");
        assert.deepStrictEqual(err.context, {});
        assert.deepStrictEqual(err.suggestions, []);
    });
    it("stores context and suggestions", () => {
        const ctx = { hook: "test", issue: "issue-1" };
        const sug = [{ summary: "restart" }];
        const err = new AutoshipError("fail", ctx, sug);
        assert.deepStrictEqual(err.context, ctx);
        assert.deepStrictEqual(err.suggestions, sug);
    });
    it("toJSON() includes all fields", () => {
        const err = new AutoshipError("fail", { hook: "h" }, [{ summary: "s" }]);
        const json = err.toJSON();
        assert.strictEqual(json.errorId, err.errorId);
        assert.strictEqual(json.message, "fail");
        assert.ok(typeof json.stack === "string");
        assert.deepStrictEqual(json.context, { hook: "h" });
        assert.deepStrictEqual(json.suggestions, [{ summary: "s" }]);
    });
    it("toString() includes id, context and suggestions", () => {
        const err = new AutoshipError("fail", { hook: "h" }, [{ summary: "s" }]);
        const str = err.toString();
        assert.ok(str.includes(err.errorId));
        assert.ok(str.includes("fail"));
        assert.ok(str.includes("h"));
        assert.ok(str.includes("s"));
    });
    it("generates unique error ids", () => {
        const a = new AutoshipError("a");
        const b = new AutoshipError("b");
        assert.notStrictEqual(a.errorId, b.errorId);
    });
});
describe("wrapError", () => {
    it("wraps a plain string", () => {
        const wrapped = wrapError("oops");
        assert.ok(wrapped instanceof AutoshipError);
        assert.strictEqual(wrapped.message, "oops");
    });
    it("wraps a native Error preserving stack", () => {
        const native = new Error("native");
        const wrapped = wrapError(native, { hook: "wrap" });
        assert.strictEqual(wrapped.message, "native");
        assert.ok(wrapped.stack?.includes("native"));
        assert.ok(wrapped.stack?.includes("--- caused by ---"));
        assert.deepStrictEqual(wrapped.context, { hook: "wrap" });
    });
    it("returns existing AutoshipError unchanged", () => {
        const original = new AutoshipError("orig", { hook: "a" }, [{ summary: "s" }]);
        const wrapped = wrapError(original);
        assert.strictEqual(wrapped.message, "orig");
        assert.deepStrictEqual(wrapped.context, { hook: "a" });
        assert.deepStrictEqual(wrapped.suggestions, [{ summary: "s" }]);
    });
    it("merges context when wrapping an AutoshipError", () => {
        const original = new AutoshipError("orig", { hook: "a", issue: "i1" });
        const wrapped = wrapError(original, { issue: "i2", category: "cat" });
        assert.deepStrictEqual(wrapped.context, { hook: "a", issue: "i2", category: "cat" });
    });
    it("concatenates suggestions when wrapping an AutoshipError", () => {
        const original = new AutoshipError("orig", {}, [{ summary: "s1" }]);
        const wrapped = wrapError(original, {}, [{ summary: "s2" }]);
        assert.deepStrictEqual(wrapped.suggestions, [{ summary: "s1" }, { summary: "s2" }]);
    });
});
describe("hookError", () => {
    it("builds an error with hook context", () => {
        const err = hookError("install", "missing file", { issue: "issue-42" });
        assert.strictEqual(err.message, "missing file");
        assert.strictEqual(err.context.hook, "install");
        assert.strictEqual(err.context.issue, "issue-42");
    });
});
describe("modelError", () => {
    it("builds an error with model metadata", () => {
        const err = modelError("gpt-4", "timeout", { issue: "issue-7" });
        assert.strictEqual(err.message, "timeout");
        assert.strictEqual(err.context.category, "model_failure");
        assert.strictEqual(err.context.metadata.model, "gpt-4");
        assert.strictEqual(err.context.issue, "issue-7");
    });
});
