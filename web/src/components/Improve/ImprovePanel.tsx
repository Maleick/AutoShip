import React, { useState } from "react";
import {
  Session,
  Suggestion,
  ChecklistItem,
  LogEvent,
  PersonalBest,
  UIState,
} from "./types";
import { SessionList } from "./SessionList";
import { SessionDetail } from "./SessionDetail";
import { HighlightMoment } from "./HighlightReel";

interface ImprovePanelProps {
  sessions?: Session[];
  suggestions?: Map<string, Suggestion[]>; // session ID → suggestions
  checklists?: Map<string, ChecklistItem[]>; // session ID → checklist
  logEvents?: Map<string, LogEvent[]>; // session ID → events
  highlights?: Map<string, HighlightMoment[]>; // session ID → highlights
  personalBests?: PersonalBest[];
  noiseFloor?: number; // default 0.01 (1%)
}

export const ImprovePanel: React.FC<ImprovePanelProps> = ({
  sessions = [],
  suggestions = new Map(),
  checklists = new Map(),
  logEvents = new Map(),
  highlights = new Map(),
  personalBests = [],
  noiseFloor = 0.01,
}) => {
  const [selectedSession, setSelectedSession] = useState<Session | null>(null);

  if (selectedSession) {
    return (
      <SessionDetail
        session={selectedSession}
        suggestions={suggestions.get(selectedSession.id) ?? []}
        checklistItems={checklists.get(selectedSession.id) ?? []}
        logEvents={logEvents.get(selectedSession.id) ?? []}
        highlights={highlights.get(selectedSession.id) ?? []}
        personalBests={personalBests}
        onBack={() => setSelectedSession(null)}
      />
    );
  }

  return (
    <SessionList sessions={sessions} onSessionSelect={setSelectedSession} />
  );
};

ImprovePanel.displayName = "ImprovePanel";
