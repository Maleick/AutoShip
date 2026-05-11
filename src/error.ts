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

function generateErrorId(): string {
  const ts = Date.now().toString(36);
  const seq = (++_idCounter).toString(36);
  return `err-${_idPrefix}-${ts}-${seq}`;
}

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
export class AutoshipError extends Error {
  /** Stable correlation ID for this error instance. */
  readonly errorId: string;
  /** Structured context captured at throw time. */
  readonly context: ErrorContext;
  /** Actionable suggestions for the operator. */
  readonly suggestions: ErrorSuggestion[];
  /** ISO-8601 timestamp when the error was created. */
  readonly timestamp: string;

  constructor(
    message: string,
    context: ErrorContext = {},
    suggestions: ErrorSuggestion[] = []
  ) {
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
  toJSON(): Record<string, unknown> {
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
  toString(): string {
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
export function wrapError(
  err: unknown,
  context: ErrorContext = {},
  suggestions: ErrorSuggestion[] = []
): AutoshipError {
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
export function hookError(
  hook: string,
  message: string,
  extraContext: Omit<ErrorContext, "hook"> = {},
  suggestions: ErrorSuggestion[] = []
): AutoshipError {
  return new AutoshipError(message, { hook, ...extraContext }, suggestions);
}

/**
 * Convenience builder for model / API failures.
 */
export function modelError(
  model: string,
  message: string,
  extraContext: Omit<ErrorContext, "category"> = {},
  suggestions: ErrorSuggestion[] = []
): AutoshipError {
  return new AutoshipError(message, {
    category: "model_failure",
    metadata: { model, ...extraContext.metadata },
    ...extraContext,
  }, suggestions);
}
