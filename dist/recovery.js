/**
 * Default recovery suggestions for known error kinds.
 *
 * These suggestions are displayed when errors occur and include optional
 * one-click recovery actions.
 */
export const DEFAULT_RECOVERY_MAP = {
    network: {
        suggestion: "A network error occurred. Check your internet connection and try again.",
        action: "Reconnect",
        command: "opencode-autoship doctor",
    },
    permission: {
        suggestion: "A permission error occurred. Verify your account credentials and access rights.",
        action: "Check account",
        command: "gh auth status",
    },
    timeout: {
        suggestion: "The operation timed out. You may retry the operation now.",
        action: "Retry",
        command: "opencode-autoship doctor",
    },
    unknown: {
        suggestion: "An unexpected error occurred. Run diagnostics for more details.",
        action: "Run diagnostics",
        command: "opencode-autoship doctor",
    },
};
/**
 * Classify an error into a known {@link ErrorKind} based on its message and
 * optional error code.
 *
 * @param message - The error message or description.
 * @param code    - Optional machine-readable error code (e.g. `ECONNREFUSED`).
 * @returns The classified {@link ErrorKind}.
 */
export function classifyError(message, code) {
    const msg = message.toLowerCase();
    const errCode = (code ?? "").toLowerCase();
    if (errCode.startsWith("econn") ||
        errCode === "enetunreach" ||
        errCode === "eai_again" ||
        msg.includes("network") ||
        msg.includes("connection") ||
        msg.includes("unreachable") ||
        msg.includes("dns") ||
        msg.includes("socket")) {
        return "network";
    }
    if (errCode === "eacces" ||
        errCode === "eperm" ||
        errCode === "enoauth" ||
        msg.includes("permission") ||
        msg.includes("unauthorized") ||
        msg.includes("forbidden") ||
        msg.includes("access denied")) {
        return "permission";
    }
    if (errCode === "etimedout" ||
        errCode === "etime" ||
        msg.includes("timeout") ||
        msg.includes("timed out") ||
        msg.includes("deadline exceeded")) {
        return "timeout";
    }
    return "unknown";
}
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
export function getRecoverySuggestion(kind, map = DEFAULT_RECOVERY_MAP) {
    return map[kind] ?? map.unknown;
}
/**
 * Format a recovery suggestion for display in the terminal.
 *
 * @param suggestion - The recovery suggestion to format.
 * @returns A human-readable string with the suggestion and optional action.
 */
export function formatRecoverySuggestion(suggestion) {
    let out = `💡 ${suggestion.suggestion}`;
    if (suggestion.action) {
        out += `\n   Action: ${suggestion.action}`;
    }
    if (suggestion.command) {
        out += `\n   Command: ${suggestion.command}`;
    }
    return out;
}
