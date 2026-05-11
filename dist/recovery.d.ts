import type { ErrorKind, RecoverySuggestion } from "./types.ts";
/**
 * Default recovery suggestions for known error kinds.
 *
 * These suggestions are displayed when errors occur and include optional
 * one-click recovery actions.
 */
export declare const DEFAULT_RECOVERY_MAP: Record<ErrorKind, RecoverySuggestion>;
/**
 * Classify an error into a known {@link ErrorKind} based on its message and
 * optional error code.
 *
 * @param message - The error message or description.
 * @param code    - Optional machine-readable error code (e.g. `ECONNREFUSED`).
 * @returns The classified {@link ErrorKind}.
 */
export declare function classifyError(message: string, code?: string): ErrorKind;
/**
 * Get a recovery suggestion for a given error kind.
 *
 * Falls back to the `unknown` suggestion if the kind is not present in the
 * provided map.
 *
 * @param kind - The classified error kind.
 * @param map  - Optional custom recovery map. Defaults to
 *               {@link DEFAULT_RECOVERY_MAP}.
 * @returns A {@link RecoverySuggestion} for the error.
 */
export declare function getRecoverySuggestion(kind: ErrorKind, map?: Record<ErrorKind, RecoverySuggestion>): RecoverySuggestion;
/**
 * Format a recovery suggestion for display in the terminal.
 *
 * @param suggestion - The recovery suggestion to format.
 * @returns A human-readable string with the suggestion and optional action.
 */
export declare function formatRecoverySuggestion(suggestion: RecoverySuggestion): string;
