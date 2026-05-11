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
/** Structured context attached to every `AutoshipError`. */
export interface ErrorContext {
    /** The hook / module where the error originated. */
    hook?: string;
    /** The issue key (e.g. `issue-3030`) related to the error. */
    issue?: string;
    /** Normalised failure category. */
    category?: string;
    /** Arbitrary key/value pairs for debugging. */
    metadata?: Record<string, unknown>;
}
/** Actionable suggestion attached to an error. */
export interface ErrorSuggestion {
    /** One-line summary of the suggestion. */
    summary: string;
    /** Optional longer explanation. */
    detail?: string;
    /** Link to relevant documentation or runbook. */
    link?: string;
}
/** AutoShip's enriched error type. */
export declare class AutoshipError extends Error {
    /** Stable correlation ID for this error instance. */
    readonly errorId: string;
    /** Structured context captured at throw time. */
    readonly context: ErrorContext;
    /** Actionable suggestions for the operator. */
    readonly suggestions: ErrorSuggestion[];
    /** ISO-8601 timestamp when the error was created. */
    readonly timestamp: string;
    constructor(message: string, context?: ErrorContext, suggestions?: ErrorSuggestion[]);
    /** Serialise the error to a plain JSON-friendly object. */
    toJSON(): Record<string, unknown>;
    /** Human-readable summary including suggestions. */
    toString(): string;
}
/**
 * Wrap an existing `Error` (or plain message) in an `AutoshipError`.
 *
 * If the input is already an `AutoshipError` it is returned unchanged
 * so that context and suggestions are not lost.
 */
export declare function wrapError(err: unknown, context?: ErrorContext, suggestions?: ErrorSuggestion[]): AutoshipError;
/**
 * Convenience builder for common hook-level errors.
 *
 * Usage:
 * ```ts
 * throw hookError("install", "opencode.json missing", { issue: "issue-3030" });
 * ```
 */
export declare function hookError(hook: string, message: string, extraContext?: Omit<ErrorContext, "hook">, suggestions?: ErrorSuggestion[]): AutoshipError;
/**
 * Convenience builder for model / API failures.
 */
export declare function modelError(model: string, message: string, extraContext?: Omit<ErrorContext, "category">, suggestions?: ErrorSuggestion[]): AutoshipError;
