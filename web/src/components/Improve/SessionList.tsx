import React, { useMemo, useState } from "react";
import { Session, SessionCampFingerprint } from "./types";
import { computeCampFingerprint, formatDuration } from "./utils";
import { Card } from "../ui/Card";
import { CampPicker } from "./CampPicker";

interface SessionListProps {
  sessions: Session[];
  onSessionSelect?: (session: Session) => void;
}

export const SessionList: React.FC<SessionListProps> = ({
  sessions,
  onSessionSelect,
}) => {
  const [selectedCamp, setSelectedCamp] = useState<SessionCampFingerprint>();

  const camps = useMemo(() => {
    const unique = new Map<string, SessionCampFingerprint>();
    sessions.forEach((s) => {
      const fp = computeCampFingerprint(s);
      unique.set(fp.campId, fp);
    });
    return Array.from(unique.values());
  }, [sessions]);

  const filtered = useMemo(() => {
    if (!selectedCamp) return sessions;
    return sessions.filter((s) => {
      const fp = computeCampFingerprint(s);
      return fp.campId === selectedCamp.campId;
    });
  }, [sessions, selectedCamp]);

  const sortedSessions = useMemo(
    () =>
      [...filtered].sort(
        (a, b) => new Date(b.date).getTime() - new Date(a.date).getTime(),
      ),
    [filtered],
  );

  return (
    <div className="space-y-4">
      <Card title="Recorded Sessions" variant="outlined">
        <CampPicker
          camps={camps}
          selected={selectedCamp}
          onSelect={setSelectedCamp}
        />
      </Card>

      {sortedSessions.length === 0 ? (
        <Card variant="outlined">
          <p className="text-xs text-white/60 text-center py-8">
            No sessions recorded yet. Run some camps to get started!
          </p>
        </Card>
      ) : (
        <div className="space-y-2">
          {sortedSessions.map((session) => {
            const fp = computeCampFingerprint(session);
            const dateStr = new Date(session.date).toLocaleDateString("en-US", {
              month: "short",
              day: "numeric",
              hour: "2-digit",
              minute: "2-digit",
            });

            return (
              <button
                key={session.id}
                onClick={() => onSessionSelect?.(session)}
                className="w-full flex items-center justify-between gap-4 p-3 sm:p-4 border border-white/10 rounded hover:border-white/30 hover:bg-white/5 transition-all text-left"
              >
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 mb-1">
                    <h3 className="font-archaic text-sm text-white truncate">
                      {session.camp.zone}
                    </h3>
                    <span className="text-xs text-white/50 flex-shrink-0">
                      {dateStr}
                    </span>
                  </div>
                  <p className="text-xs text-white/60">
                    {formatDuration(session.duration)} •{" "}
                    <span className="text-white/70">
                      {session.xpPerHour.toFixed(0)}/hr
                    </span>
                  </p>
                  <p className="text-[10px] text-white/40 mt-0.5">
                    {session.camp.mobSet.join(", ")}
                  </p>
                </div>

                <div className="flex items-center gap-3 flex-shrink-0">
                  {session.metrics.dps && (
                    <div className="text-right">
                      <p className="text-[10px] text-white/50">DPS</p>
                      <p className="text-sm font-bold text-white">
                        {session.metrics.dps.toFixed(0)}
                      </p>
                    </div>
                  )}
                  {session.metrics.healing && (
                    <div className="text-right">
                      <p className="text-[10px] text-white/50">Heal</p>
                      <p className="text-sm font-bold text-white">
                        {session.metrics.healing.toFixed(0)}
                      </p>
                    </div>
                  )}
                  {session.metrics.deaths !== undefined && (
                    <div className="text-right">
                      <p className="text-[10px] text-white/50">Deaths</p>
                      <p className="text-sm font-bold text-white">
                        {session.metrics.deaths}
                      </p>
                    </div>
                  )}
                </div>
              </button>
            );
          })}
        </div>
      )}

      <p className="text-xs text-white/40 text-center">
        {selectedCamp
          ? `${sortedSessions.length} sessions`
          : `${sessions.length} total sessions`}
      </p>
    </div>
  );
};

SessionList.displayName = "SessionList";
