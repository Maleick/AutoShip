import { useEffect, useState } from "react";
import { AlertCircle, CheckCircle, XCircle, Zap, type LucideIcon } from "lucide-react";

interface Suggestion {
  id: string;
  title: string;
  description: string;
  suggestion_type: string;
  priority: number;
  recommendation: string;
  created_at: string;
  status: "pending" | "accepted" | "rejected" | "applied" | "undone";
  config_path?: string;
  suggested_value?: unknown;
}

interface SuggestionsResponse {
  suggestions: Suggestion[];
  count: number;
}

const SUGGESTION_ICONS: Record<string, LucideIcon> = {
  performance: Zap,
  safety: AlertCircle,
  config_tuning: AlertCircle,
  behavior: AlertCircle,
  resource: Zap,
  combat_rotation: Zap,
  navigation: AlertCircle,
  other: AlertCircle,
};

const PRIORITY_COLORS: Record<number, string> = {
  1: "bg-blue-900 text-blue-100",
  2: "bg-cyan-900 text-cyan-100",
  3: "bg-yellow-900 text-yellow-100",
  4: "bg-orange-900 text-orange-100",
  5: "bg-red-900 text-red-100",
};

export function Improvement() {
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [actionInProgress, setActionInProgress] = useState<string | null>(null);

  useEffect(() => {
    fetchSuggestions();
    const interval = setInterval(fetchSuggestions, 30000); // Refresh every 30 seconds
    return () => clearInterval(interval);
  }, []);

  async function fetchSuggestions() {
    try {
      setLoading(true);
      setError(null);
      const response = await fetch("/api/improvement/suggestions", {
        headers: {
          "X-API-Token": sessionStorage.getItem("apiToken") || "",
        },
      });

      if (!response.ok) {
        if (response.status === 401) {
          setError("Authentication failed");
        } else {
          setError(`Failed to load suggestions (${response.status})`);
        }
        return;
      }

      const data: SuggestionsResponse = await response.json();
      setSuggestions(data.suggestions);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load suggestions");
    } finally {
      setLoading(false);
    }
  }

  async function handleAction(
    suggestionId: string,
    action: "accept" | "reject" | "apply" | "undo",
  ) {
    setActionInProgress(suggestionId);
    try {
      const endpoint = `/api/improvement/${action}/${suggestionId}`;
      const response = await fetch(endpoint, {
        method: "POST",
        headers: {
          "X-API-Token": sessionStorage.getItem("apiToken") || "",
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ action, note: "" }),
      });

      if (!response.ok) {
        throw new Error(`Action failed: ${response.status}`);
      }

      // Update local state
      setSuggestions((prev) =>
        prev.map((s) => {
          if (s.id === suggestionId) {
            const statusMap = {
              accept: "accepted",
              reject: "rejected",
              apply: "applied",
              undo: "undone",
            };
            return { ...s, status: statusMap[action] as Suggestion["status"] };
          }
          return s;
        }),
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : "Action failed");
    } finally {
      setActionInProgress(null);
    }
  }

  const pendingSuggestions = suggestions.filter((s) => s.status === "pending");
  const resolvedSuggestions = suggestions.filter((s) => s.status !== "pending");

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div>
        <h1 className="text-2xl font-bold text-neriak-magenta mb-2">Self-Improvement</h1>
        <p className="text-neriak-muted text-sm">
          Operator-in-the-loop tuning suggestions based on session analysis.
        </p>
      </div>

      {/* Error message */}
      {error && (
        <div className="p-4 rounded-lg border border-red-500/30 bg-red-500/5 text-red-400 text-sm">
          {error}
        </div>
      )}

      {/* Loading state */}
      {loading && (
        <div className="p-6 text-center text-neriak-muted">
          <div className="inline-block animate-spin">⟳</div> Loading suggestions...
        </div>
      )}

      {/* Empty state */}
      {!loading && suggestions.length === 0 && (
        <div className="p-6 text-center border border-neriak-dim rounded-lg">
          <p className="text-neriak-muted mb-2">No suggestions available.</p>
          <p className="text-sm text-neriak-dim">
            Suggestions will appear after session analysis completes.
          </p>
        </div>
      )}

      {/* Pending Suggestions */}
      {!loading && pendingSuggestions.length > 0 && (
        <div className="space-y-3">
          <h2 className="text-lg font-semibold text-neriak-text">
            Pending Suggestions ({pendingSuggestions.length})
          </h2>
          <div className="space-y-3">
            {pendingSuggestions.map((suggestion) => (
              <SuggestionCard
                key={suggestion.id}
                suggestion={suggestion}
                onAction={(action) => handleAction(suggestion.id, action)}
                isLoading={actionInProgress === suggestion.id}
              />
            ))}
          </div>
        </div>
      )}

      {/* Resolved Suggestions */}
      {!loading && resolvedSuggestions.length > 0 && (
        <div className="space-y-3">
          <h2 className="text-lg font-semibold text-neriak-text">
            History ({resolvedSuggestions.length})
          </h2>
          <div className="space-y-2 max-h-96 overflow-y-auto">
            {resolvedSuggestions.map((suggestion) => (
              <div
                key={suggestion.id}
                className="p-3 rounded border border-neriak-dim/30 bg-panel/30 text-sm"
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <StatusIcon status={suggestion.status} />
                    <span className="text-neriak-text font-mono text-xs uppercase">
                      {suggestion.status}
                    </span>
                    <span className="text-neriak-muted">{suggestion.title}</span>
                  </div>
                  <span className="text-neriak-dim text-xs">
                    {new Date(suggestion.created_at).toLocaleTimeString()}
                  </span>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

interface SuggestionCardProps {
  suggestion: Suggestion;
  onAction: (action: "accept" | "reject" | "apply" | "undo") => Promise<void>;
  isLoading: boolean;
}

function SuggestionCard({ suggestion, onAction, isLoading }: SuggestionCardProps) {
  const Icon = SUGGESTION_ICONS[suggestion.suggestion_type] || AlertCircle;
  const priorityClass = PRIORITY_COLORS[suggestion.priority] || PRIORITY_COLORS[1];

  return (
    <div className="p-4 rounded-lg border border-neriak-dim bg-elevated hover:bg-elevated/80 transition-colors">
      <div className="flex gap-4">
        <div className="flex-shrink-0 pt-1">
          <Icon className="w-5 h-5 text-neriak-magenta" strokeWidth={1.5} />
        </div>

        <div className="flex-1 min-w-0">
          <div className="flex items-start justify-between gap-2 mb-2">
            <div>
              <h3 className="font-semibold text-neriak-text break-words">{suggestion.title}</h3>
              <p className="text-xs text-neriak-muted mt-1 capitalize">
                {suggestion.suggestion_type.replace(/_/g, " ")}
              </p>
            </div>
            <div className={`flex-shrink-0 px-2 py-1 rounded text-xs font-mono ${priorityClass}`}>
              P{suggestion.priority}
            </div>
          </div>

          <p className="text-sm text-neriak-text/80 mb-3">{suggestion.description}</p>

          <div className="p-3 bg-void/50 rounded border border-neriak-dim/30 mb-4">
            <p className="text-xs font-mono text-neriak-muted mb-1">Recommendation:</p>
            <p className="text-sm text-neriak-text break-words">{suggestion.recommendation}</p>
            {suggestion.config_path && (
              <p className="text-xs text-neriak-dim mt-2">
                Config: <span className="font-mono">{suggestion.config_path}</span>
              </p>
            )}
          </div>

          <div className="flex gap-2">
            <button
              onClick={() => onAction("accept")}
              disabled={isLoading}
              className="px-3 py-1 rounded text-xs font-mono uppercase bg-green-900/30 text-green-300 border border-green-500/30 hover:bg-green-900/50 disabled:opacity-50 transition-colors"
            >
              {isLoading ? "..." : "Accept"}
            </button>
            {suggestion.config_path && (
              <button
                onClick={() => onAction("apply")}
                disabled={isLoading}
                className="px-3 py-1 rounded text-xs font-mono uppercase bg-blue-900/30 text-blue-300 border border-blue-500/30 hover:bg-blue-900/50 disabled:opacity-50 transition-colors"
              >
                {isLoading ? "..." : "Apply"}
              </button>
            )}
            <button
              onClick={() => onAction("reject")}
              disabled={isLoading}
              className="px-3 py-1 rounded text-xs font-mono uppercase bg-red-900/30 text-red-300 border border-red-500/30 hover:bg-red-900/50 disabled:opacity-50 transition-colors"
            >
              {isLoading ? "..." : "Reject"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

function StatusIcon({ status }: { status: string }) {
  switch (status) {
    case "accepted":
      return <CheckCircle className="w-4 h-4 text-green-400" strokeWidth={2} />;
    case "applied":
      return <CheckCircle className="w-4 h-4 text-green-400" strokeWidth={2} />;
    case "rejected":
      return <XCircle className="w-4 h-4 text-red-400" strokeWidth={2} />;
    case "undone":
      return <XCircle className="w-4 h-4 text-red-400" strokeWidth={2} />;
    default:
      return <AlertCircle className="w-4 h-4 text-yellow-400" strokeWidth={2} />;
  }
}
