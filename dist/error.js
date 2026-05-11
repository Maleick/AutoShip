/**
 * Error context utilities for AutoShip.
 *
 * Provides a lightweight `AutoshipError` class that carries:
 * - stack trace (captured at throw time)
 * - contextual metadata (hook, issue, category)
 * - actionable suggestions
 * - stable error IDs for correlation
 *
 * Designed for zero-overhead when not thrown: the class is a thin
 * wrapper around the native `Error` with extra fields.
 */
let _idCounter = 0;
const _idPrefix = Math.random().toString(36).slice(2, 8);
function generateErrorId() {
    const ts = Date.now().toString(36);
    const seq = (++_idCounter).toString(36);
    return `err-${_idPrefix}-${ts}-${seq}`;
}
/** AutoShip's enriched error type. */
export class AutoshipError extends Error {
    /** Stable correlation ID for this error instance. */
    errorId;
    /** Structured context captured at throw time. */
    context;
    /** Actionable suggestions for the operator. */
    suggestions;
    /** ISO-8601 timestamp when the error was created. */
    timestamp;
    constructor(message, context = {}, suggestions = []) {
        super(message);
        this.name = "AutoshipError";
        this.errorId = generateErrorId();
        this.context = { ...context };
        this.suggestions = [...suggestions];
        this.timestamp = new Date().toISOString();
        // Ensure the stack trace is captured on this instance
        if (Error.captureStackTrace) {
            Error.captureStackTrace(this, AutoshipError);
        }
    }
    /** Serialise the error to a plain JSON-friendly object. */
    toJSON() {
        return {
            errorId: this.errorId,
            name: this.name,
            message: this.message,
            stack: this.stack,
            timestamp: this.timestamp,
            context: this.context,
            suggestions: this.suggestions,
        };
    }
    /** Human-readable summary including suggestions. */
    toString() {
        const suggestionLines = this.suggestions
            .map((s, i) => `  ${i + 1}. ${s.summary}${s.detail ? ` — ${s.detail}` : ""}`)
            .join("\n");
        return [
            `[${this.errorId}] ${this.name}: ${this.message}`,
            `  context: ${JSON.stringify(this.context)}`,
            suggestionLines ? `  suggestions:\n${suggestionLines}` : "",
            this.stack ? `  stack:\n${this.stack}` : "",
        ]
            .filter(Boolean)
            .join("\n");
    }
}
/**
 * Wrap an existing `Error` (or plain message) in an `AutoshipError`.
 *
 * If the input is already an `AutoshipError` it is returned unchanged
 * so that context and suggestions are not lost.
 */
export function wrapError(err, context = {}, suggestions = []) {
    if (err instanceof AutoshipError) {
        // Merge new context / suggestions into the existing error
        return new AutoshipError(err.message, {
            ...err.context,
            ...context,
        }, [...err.suggestions, ...suggestions]);
    }
    const message = err instanceof Error ? err.message : String(err);
    const wrapped = new AutoshipError(message, context, suggestions);
    if (err instanceof Error && err.stack) {
        // Preserve original stack as the first frame, then append our wrapper
        wrapped.stack = `${wrapped.stack}\n--- caused by ---\n${err.stack}`;
    }
    return wrapped;
}
/**
 * Convenience builder for common hook-level errors.
 *
 * Usage:
 * ```ts
 * throw hookError("install", "opencode.json missing", { issue: "issue-3030" });
 * ```
 */
export function hookError(hook, message, extraContext = {}, suggestions = []) {
    return new AutoshipError(message, { hook, ...extraContext }, suggestions);
}
/**
 * Convenience builder for model / API failures.
 */
export function modelError(model, message, extraContext = {}, suggestions = []) {
    return new AutoshipError(message, {
        category: "model_failure",
        metadata: { model, ...extraContext.metadata },
        ...extraContext,
    }, suggestions);
}
