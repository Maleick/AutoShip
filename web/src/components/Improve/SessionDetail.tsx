import React, { useState } from "react";
import {
  Session,
  Suggestion,
  ChecklistItem,
  ViewMode,
  LogEvent,
  PersonalBest,
} from "./types";
import {
  computeCampFingerprint,
  formatDuration,
  findPersonalBest,
} from "./utils";
import { Card } from "../ui/Card";
import { TabBar } from "./TabBar";
import { SeverityToggle } from "./SeverityToggle";
import { Checklist } from "./Checklist";
import { SuggestionCard } from "./SuggestionCard";
import { Timeline } from "./Timeline";
import { EventList } from "./EventList";
import { ComparisonChart } from "./ComparisonChart";
import { MetricCard } from "./MetricCard";
import { HighlightReel, HighlightMoment } from "./HighlightReel";
import { TimelineScrubber } from "./TimelineScrubber";

interface SessionDetailProps {
  session: Session;
  suggestions?: Suggestion[];
  checklistItems?: ChecklistItem[];
  logEvents?: LogEvent[];
  highlights?: HighlightMoment[];
  personalBests?: PersonalBest[];
  onBack?: () => void;
}

type TabType = "dps" | "healing" | "cc" | "deaths" | "resources" | "pulls";

export const SessionDetail: React.FC<SessionDetailProps> = ({
  session,
  suggestions = [],
  checklistItems = [],
  logEvents = [],
  highlights = [],
  personalBests = [],
  onBack,
}) => {
  const [viewMode, setViewMode] = useState<ViewMode>("tables");
  const [showMinor, setShowMinor] = useState(false);
  const [activeTab, setActiveTab] = useState<TabType>("dps");
  const [currentTime, setCurrentTime] = useState(0);
  const [selectedCharacter, setSelectedCharacter] = useState<string>();

  const campFingerprint = computeCampFingerprint(session);
  const pbXph = findPersonalBest(
    personalBests,
    campFingerprint.campId,
    "xphour",
  );
  const pbDps = findPersonalBest(personalBests, campFingerprint.campId, "dps");

  const filterSuggestions = (severity: "major" | "average" | "minor") =>
    suggestions.filter((s) => s.severity === severity);
  const majorSuggestions = filterSuggestions("major");
  const avgSuggestions = filterSuggestions("average");
  const minorSuggestions = filterSuggestions("minor");

  const visibleSuggestions = [
    ...majorSuggestions,
    ...avgSuggestions,
    ...(showMinor ? minorSuggestions : []),
  ];

  const characters = [...new Set(logEvents.map((e) => e.actor))];

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-start justify-between gap-4">
        <div>
          {onBack && (
            <button
              onClick={onBack}
              className="text-xs text-blue-400 hover:text-blue-300 mb-2"
            >
              ← Back to Sessions
            </button>
          )}
          <h2 className="font-archaic text-xl text-white">
            {session.camp.zone} • {formatDuration(session.duration)}
          </h2>
          <p className="text-xs text-white/60 mt-1">
            {new Date(session.date).toLocaleDateString()} •{" "}
            {session.xpPerHour.toFixed(0)}/hr
          </p>
        </div>
      </div>

      {/* Key metrics */}
      <div className="grid grid-cols-2 sm:grid-cols-3 gap-2">
        <MetricCard
          label="XP/Hour"
          current={session.xpPerHour}
          personalBest={pbXph?.value}
          metricType="xphour"
        />
        {session.metrics.dps && (
          <MetricCard
            label="DPS"
            current={session.metrics.dps}
            personalBest={pbDps?.value}
            metricType="dps"
          />
        )}
        {session.metrics.healing && (
          <MetricCard
            label="Healing"
            current={session.metrics.healing}
            metricType="healing"
          />
        )}
        {session.metrics.deaths !== undefined && (
          <MetricCard
            label="Deaths"
            current={session.metrics.deaths}
            metricType="percent"
          />
        )}
      </div>

      {/* Highlights */}
      {highlights.length > 0 && <HighlightReel moments={highlights} />}

      {/* Checklist */}
      {checklistItems.length > 0 && <Checklist items={checklistItems} />}

      {/* Suggestions */}
      {visibleSuggestions.length > 0 && (
        <Card title="Suggestions" variant="outlined">
          <div className="space-y-3">
            <SeverityToggle
              showMinor={showMinor}
              onToggle={setShowMinor}
              minorCount={minorSuggestions.length}
            />

            <div className="space-y-2">
              {visibleSuggestions.map((sug) => (
                <SuggestionCard
                  key={sug.id}
                  title={sug.title}
                  severity={sug.severity}
                  evidence={sug.evidence}
                  deltaValue={sug.deltaValue}
                  deltaType={sug.deltaType}
                />
              ))}
            </div>
          </div>
        </Card>
      )}

      {/* Views: Tables / Timeline / Events */}
      {logEvents.length > 0 && (
        <>
          <div className="flex gap-2 items-center">
            <button
              onClick={() => setViewMode("tables")}
              className={`px-3 py-1 text-xs rounded border transition-all ${
                viewMode === "tables"
                  ? "border-white/40 bg-white/10 text-white"
                  : "border-white/10 text-white/60 hover:border-white/20"
              }`}
            >
              Tables
            </button>
            <button
              onClick={() => setViewMode("timeline")}
              className={`px-3 py-1 text-xs rounded border transition-all ${
                viewMode === "timeline"
                  ? "border-white/40 bg-white/10 text-white"
                  : "border-white/10 text-white/60 hover:border-white/20"
              }`}
            >
              Timeline
            </button>
            <button
              onClick={() => setViewMode("events")}
              className={`px-3 py-1 text-xs rounded border transition-all ${
                viewMode === "events"
                  ? "border-white/40 bg-white/10 text-white"
                  : "border-white/10 text-white/60 hover:border-white/20"
              }`}
            >
              Events
            </button>
          </div>

          {(viewMode === "tables" || viewMode === "timeline") &&
            characters.length > 0 && (
              <TabBar activeTab={activeTab} onTabChange={setActiveTab} />
            )}

          {viewMode === "timeline" && (
            <>
              <Timeline
                events={logEvents}
                characters={characters}
                selectedCharacter={selectedCharacter}
                onCharacterSelect={setSelectedCharacter}
              />
              <TimelineScrubber
                currentTime={currentTime}
                maxTime={session.duration}
                onTimeChange={setCurrentTime}
              />
            </>
          )}

          {viewMode === "events" && <EventList events={logEvents} />}

          {viewMode === "tables" && (
            <Card
              title="Statistics"
              subtitle="Aggregated by tab"
              variant="outlined"
            >
              <p className="text-xs text-white/60">
                {activeTab.toUpperCase()}: {logEvents.length} events •{" "}
                {characters.length} characters
              </p>
            </Card>
          )}
        </>
      )}
    </div>
  );
};

SessionDetail.displayName = "SessionDetail";
