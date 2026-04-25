import { useState } from "react";
import { PageHeader } from "../components/PageHeader";
import { SuggestionCard } from "../components/SuggestionCard";
import type { Suggestion } from "../types/suggestion";
import { MOCK_SUGGESTIONS } from "../lib/mocks";

export function Suggestions() {
  const [suggestions, setSuggestions] = useState<Suggestion[]>(MOCK_SUGGESTIONS);

  const handleAccept = (id: string) => {
    setSuggestions((prev) => prev.map((s) => (s.id === id ? { ...s, status: "accepted" } : s)));
  };

  const handleDismiss = (id: string) => {
    setSuggestions((prev) => prev.map((s) => (s.id === id ? { ...s, status: "dismissed" } : s)));
  };

  const handleDefer = (id: string) => {
    setSuggestions((prev) => prev.map((s) => (s.id === id ? { ...s, status: "deferred" } : s)));
  };

  const pending = suggestions.filter((s) => s.status === "pending");
  const resolved = suggestions.filter((s) => s.status !== "pending");

  return (
    <div className="space-y-6">
      <PageHeader
        title="Tuning Suggestions"
        subtitle="Fleet optimization recommendations based on current state"
      />

      {pending.length === 0 ? (
        <div className="bg-panel border border-neriak-dim rounded-md p-8 text-center">
          <p className="text-sm text-neriak-muted">No pending suggestions</p>
        </div>
      ) : (
        <div className="space-y-3">
          <h3 className="text-xs font-semibold text-neriak-muted uppercase tracking-[0.18em]">
            Pending ({pending.length})
          </h3>
          <div className="space-y-3">
            {pending.map((suggestion) => (
              <SuggestionCard
                key={suggestion.id}
                suggestion={suggestion}
                onAccept={handleAccept}
                onDismiss={handleDismiss}
                onDefer={handleDefer}
              />
            ))}
          </div>
        </div>
      )}

      {resolved.length > 0 && (
        <div className="space-y-3">
          <h3 className="text-xs font-semibold text-neriak-muted uppercase tracking-[0.18em]">
            Resolved ({resolved.length})
          </h3>
          <div className="space-y-3">
            {resolved.map((suggestion) => (
              <SuggestionCard
                key={suggestion.id}
                suggestion={suggestion}
                onAccept={handleAccept}
                onDismiss={handleDismiss}
                onDefer={handleDefer}
                disabled
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
