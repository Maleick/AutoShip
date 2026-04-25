import {
  CheckCircle,
  XCircle,
  Clock,
  FolderPlus,
  ArrowClockwise,
} from "@phosphor-icons/react";
import { useEffect, useState } from "react";

/**
 * Session debrief card showing top wins, losses, and suggestions.
 */
interface SessionDebrief {
  session_id: string;
  character_id: string;
  started_at: string;
  ended_at: string;
  top_wins: string[];
  top_losses: string[];
  suggestions: Suggestion[];
}

/**
 * A single actionable suggestion for a character.
 */
interface Suggestion {
  id: string;
  title: string;
  current_value: string;
  proposed_value: string;
  rationale: string;
  confidence: number;
  config_key: string;
}

/**
 * Tracks accepted suggestions that can be undone.
 */
interface AcceptedSuggestion {
  id: string;
  prior_value: string;
  character_id: string;
}

const buttonClasses =
  "px-3 py-1 text-xs rounded border transition-colors font-medium";
const primaryButton =
  "bg-emerald-600 hover:bg-emerald-700 text-white border-emerald-500";
const secondaryButton =
  "bg-red-600/20 hover:bg-red-600/30 text-red-300 border-red-500/30";
const tertiaryButton =
  "bg-amber-600/20 hover:bg-amber-600/30 text-amber-300 border-amber-500/30";
const quaternaryButton =
  "bg-blue-600/20 hover:bg-blue-600/30 text-blue-300 border-blue-500/30";

/**
 * ImprovePanel — Operator-facing UI for reviewing and acting on session suggestions.
 *
 * Renders per-character debrief card with wins, losses, and suggestions.
 * Each suggestion can be accepted, rejected, snoozed, or promoted to global.
 * Accepted suggestions can be undone by restoring the prior config snapshot.
 */
export default function ImprovePanel() {
  const [debrief, setDebrief] = useState<SessionDebrief | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [acceptedSuggestions, setAcceptedSuggestions] = useState<
    AcceptedSuggestion[]
  >([]);
  const [selectedCharacter, setSelectedCharacter] = useState<string>("");

  /**
   * Fetch session debrief for the selected character.
   */
  useEffect(() => {
    if (!selectedCharacter) return;

    const fetchDebrief = async () => {
      try {
        setLoading(true);
        setError(null);

        const response = await fetch(
          `/api/improve/sessions/${encodeURIComponent(selectedCharacter)}`,
          {
            headers: {
              "X-API-Token": import.meta.env.VITE_API_TOKEN || "",
            },
          },
        );

        if (response.status === 404) {
          setError("No closed sessions available for this character.");
          setDebrief(null);
          return;
        }

        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }

        const data: SessionDebrief = await response.json();
        setDebrief(data);
      } catch (err) {
        setError(`Failed to load debrief: ${err}`);
        setDebrief(null);
      } finally {
        setLoading(false);
      }
    };

    fetchDebrief();
  }, [selectedCharacter]);

  /**
   * Accept a suggestion — write to character config + audit log.
   */
  const handleAccept = async (suggestion: Suggestion) => {
    try {
      const response = await fetch(
        `/api/improve/suggestions/${encodeURIComponent(suggestion.id)}/accept`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "X-API-Token": import.meta.env.VITE_API_TOKEN || "",
          },
          body: JSON.stringify({
            character_id: debrief?.character_id,
            value: suggestion.proposed_value,
          }),
        },
      );

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      // Record for undo capability
      setAcceptedSuggestions([
        ...acceptedSuggestions,
        {
          id: suggestion.id,
          prior_value: suggestion.current_value,
          character_id: debrief?.character_id || "",
        },
      ]);

      // Refresh debrief
      if (selectedCharacter) {
        const refreshResp = await fetch(
          `/api/improve/sessions/${encodeURIComponent(selectedCharacter)}`,
          {
            headers: {
              "X-API-Token": import.meta.env.VITE_API_TOKEN || "",
            },
          },
        );
        if (refreshResp.ok) {
          setDebrief(await refreshResp.json());
        }
      }
    } catch (err) {
      setError(`Failed to accept suggestion: ${err}`);
    }
  };

  /**
   * Reject a suggestion so it doesn't re-emit.
   */
  const handleReject = async (suggestion: Suggestion) => {
    try {
      const response = await fetch(
        `/api/improve/suggestions/${encodeURIComponent(suggestion.id)}/reject`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "X-API-Token": import.meta.env.VITE_API_TOKEN || "",
          },
          body: JSON.stringify({
            character_id: debrief?.character_id,
          }),
        },
      );

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      // Refresh debrief
      if (selectedCharacter) {
        const refreshResp = await fetch(
          `/api/improve/sessions/${encodeURIComponent(selectedCharacter)}`,
          {
            headers: {
              "X-API-Token": import.meta.env.VITE_API_TOKEN || "",
            },
          },
        );
        if (refreshResp.ok) {
          setDebrief(await refreshResp.json());
        }
      }
    } catch (err) {
      setError(`Failed to reject suggestion: ${err}`);
    }
  };

  /**
   * Snooze a suggestion for 1 session.
   */
  const handleSnooze = async (suggestion: Suggestion) => {
    try {
      const response = await fetch(
        `/api/improve/suggestions/${encodeURIComponent(suggestion.id)}/snooze`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "X-API-Token": import.meta.env.VITE_API_TOKEN || "",
          },
          body: JSON.stringify({
            character_id: debrief?.character_id,
            sessions: 1,
          }),
        },
      );

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      // Refresh debrief
      if (selectedCharacter) {
        const refreshResp = await fetch(
          `/api/improve/sessions/${encodeURIComponent(selectedCharacter)}`,
          {
            headers: {
              "X-API-Token": import.meta.env.VITE_API_TOKEN || "",
            },
          },
        );
        if (refreshResp.ok) {
          setDebrief(await refreshResp.json());
        }
      }
    } catch (err) {
      setError(`Failed to snooze suggestion: ${err}`);
    }
  };

  /**
   * Promote a suggestion to all characters of the same class.
   */
  const handlePromote = async (suggestion: Suggestion) => {
    try {
      const response = await fetch(
        `/api/improve/suggestions/${encodeURIComponent(suggestion.id)}/promote`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "X-API-Token": import.meta.env.VITE_API_TOKEN || "",
          },
          body: JSON.stringify({
            character_id: debrief?.character_id,
            value: suggestion.proposed_value,
          }),
        },
      );

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      // Refresh debrief
      if (selectedCharacter) {
        const refreshResp = await fetch(
          `/api/improve/sessions/${encodeURIComponent(selectedCharacter)}`,
          {
            headers: {
              "X-API-Token": import.meta.env.VITE_API_TOKEN || "",
            },
          },
        );
        if (refreshResp.ok) {
          setDebrief(await refreshResp.json());
        }
      }
    } catch (err) {
      setError(`Failed to promote suggestion: ${err}`);
    }
  };

  /**
   * Undo a previously accepted suggestion.
   */
  const handleUndo = async (acceptedSugg: AcceptedSuggestion) => {
    try {
      const response = await fetch(
        `/api/improve/suggestions/${encodeURIComponent(acceptedSugg.id)}/undo`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "X-API-Token": import.meta.env.VITE_API_TOKEN || "",
          },
          body: JSON.stringify({
            character_id: acceptedSugg.character_id,
            prior_value: acceptedSugg.prior_value,
          }),
        },
      );

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      // Remove from accepted list
      setAcceptedSuggestions(
        acceptedSuggestions.filter((s) => s.id !== acceptedSugg.id),
      );

      // Refresh debrief
      if (selectedCharacter) {
        const refreshResp = await fetch(
          `/api/improve/sessions/${encodeURIComponent(selectedCharacter)}`,
          {
            headers: {
              "X-API-Token": import.meta.env.VITE_API_TOKEN || "",
            },
          },
        );
        if (refreshResp.ok) {
          setDebrief(await refreshResp.json());
        }
      }
    } catch (err) {
      setError(`Failed to undo suggestion: ${err}`);
    }
  };

  return (
    <div className="space-y-4">
      <h2 className="text-2xl font-archaic text-white">Self-Improvement</h2>

      {/* Character selector (placeholder for now) */}
      <div className="border border-white/10 bg-black/20 px-4 py-3">
        <label className="block text-xs uppercase tracking-[0.3em] text-white/50 font-rune mb-2">
          Select Character
        </label>
        <input
          type="text"
          value={selectedCharacter}
          onChange={(e) => setSelectedCharacter(e.target.value)}
          placeholder="Enter character name"
          className="w-full bg-black/40 border border-white/20 text-white px-3 py-2 text-sm"
        />
      </div>

      {/* Error message */}
      {error && (
        <div className="border border-red-500/30 bg-red-500/10 px-4 py-3 text-red-300 text-sm">
          {error}
        </div>
      )}

      {/* Loading state */}
      {loading && selectedCharacter && (
        <div className="text-center py-8 text-white/50">Loading debrief...</div>
      )}

      {/* Debrief card */}
      {debrief && !loading && (
        <div className="border border-white/10 bg-black/20 px-4 py-4 space-y-4">
          <div>
            <h3 className="text-sm uppercase tracking-[0.3em] text-white/50 font-rune mb-2">
              Session {debrief.session_id}
            </h3>
            <p className="text-xs text-white/40">
              {new Date(debrief.started_at).toLocaleString()} -{" "}
              {new Date(debrief.ended_at).toLocaleString()}
            </p>
          </div>

          {/* Top 3 Wins */}
          <div>
            <h4 className="text-xs uppercase tracking-[0.2em] text-emerald-300 font-rune mb-2">
              ✓ Top Wins
            </h4>
            <ul className="space-y-1 text-sm text-white/70">
              {debrief.top_wins.map((win, idx) => (
                <li key={idx} className="flex gap-2">
                  <span className="text-emerald-400">•</span>
                  <span>{win}</span>
                </li>
              ))}
            </ul>
          </div>

          {/* Top 3 Losses */}
          <div>
            <h4 className="text-xs uppercase tracking-[0.2em] text-red-300 font-rune mb-2">
              ✗ Top Losses
            </h4>
            <ul className="space-y-1 text-sm text-white/70">
              {debrief.top_losses.map((loss, idx) => (
                <li key={idx} className="flex gap-2">
                  <span className="text-red-400">•</span>
                  <span>{loss}</span>
                </li>
              ))}
            </ul>
          </div>

          {/* Suggestions */}
          {debrief.suggestions.length > 0 && (
            <div>
              <h4 className="text-xs uppercase tracking-[0.2em] text-cyan-300 font-rune mb-3">
                Suggestions ({debrief.suggestions.length})
              </h4>
              <div className="space-y-3">
                {debrief.suggestions.map((suggestion) => {
                  const accepted = acceptedSuggestions.find(
                    (s) => s.id === suggestion.id,
                  );
                  return (
                    <div
                      key={suggestion.id}
                      className="border border-white/10 bg-black/40 px-3 py-3 space-y-2"
                    >
                      <div className="flex justify-between items-start">
                        <div className="flex-1">
                          <h5 className="font-medium text-white text-sm">
                            {suggestion.title}
                          </h5>
                          <p className="text-xs text-white/50 mt-1">
                            {suggestion.config_key}
                          </p>
                          <p className="text-sm text-white/70 mt-2">
                            {suggestion.current_value} →{" "}
                            <span className="text-emerald-300">
                              {suggestion.proposed_value}
                            </span>
                          </p>
                          <p className="text-xs text-white/60 mt-2 italic">
                            {suggestion.rationale}
                          </p>
                          <div className="mt-2">
                            <span className="text-xs text-white/50">
                              Confidence:{" "}
                              <span className="text-cyan-300">
                                {(suggestion.confidence * 100).toFixed(0)}%
                              </span>
                            </span>
                          </div>
                        </div>
                      </div>

                      {/* Action buttons */}
                      <div className="flex gap-2 pt-2">
                        <button
                          onClick={() => handleAccept(suggestion)}
                          className={`${buttonClasses} ${primaryButton}`}
                          title="Accept this suggestion"
                        >
                          <CheckCircle
                            size={14}
                            className="inline mr-1"
                            weight="fill"
                          />
                          Accept
                        </button>
                        <button
                          onClick={() => handleReject(suggestion)}
                          className={`${buttonClasses} ${secondaryButton}`}
                          title="Reject this suggestion"
                        >
                          <XCircle
                            size={14}
                            className="inline mr-1"
                            weight="fill"
                          />
                          Reject
                        </button>
                        <button
                          onClick={() => handleSnooze(suggestion)}
                          className={`${buttonClasses} ${tertiaryButton}`}
                          title="Snooze for 1 session"
                        >
                          <Clock
                            size={14}
                            className="inline mr-1"
                            weight="fill"
                          />
                          Snooze
                        </button>
                        <button
                          onClick={() => handlePromote(suggestion)}
                          className={`${buttonClasses} ${quaternaryButton}`}
                          title="Apply to all characters of this class"
                        >
                          <FolderPlus
                            size={14}
                            className="inline mr-1"
                            weight="fill"
                          />
                          Global
                        </button>
                        {accepted && (
                          <button
                            onClick={() => handleUndo(accepted)}
                            className={`${buttonClasses} bg-orange-600/20 hover:bg-orange-600/30 text-orange-300 border-orange-500/30`}
                            title="Undo this accepted suggestion"
                          >
                            <ArrowClockwise
                              size={14}
                              className="inline mr-1"
                              weight="fill"
                            />
                            Undo
                          </button>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
