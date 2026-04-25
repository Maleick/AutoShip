import React from "react";
import { Card } from "../ui/Card";
import { LogEvent } from "./types";

interface TimelineProps {
  events: LogEvent[];
  characters?: string[];
  selectedCharacter?: string;
  onCharacterSelect?: (char: string) => void;
  onEventClick?: (event: LogEvent) => void;
}

export const Timeline: React.FC<TimelineProps> = ({
  events,
  characters = [],
  selectedCharacter,
  onCharacterSelect,
  onEventClick,
}) => {
  const filteredEvents = selectedCharacter
    ? events.filter((e) => e.actor === selectedCharacter)
    : events;

  const maxTime = Math.max(...events.map((e) => e.timestamp), 1);

  return (
    <Card
      title="Timeline View"
      subtitle="v1: functional, minimal styling"
      variant="outlined"
    >
      {characters.length > 0 && (
        <div className="flex gap-1 mb-4 flex-wrap">
          {characters.map((char) => (
            <button
              key={char}
              onClick={() => onCharacterSelect?.(char)}
              className={`px-2 py-1 text-xs border rounded transition-all ${
                selectedCharacter === char
                  ? "border-white/40 bg-white/10 text-white"
                  : "border-white/10 text-white/60 hover:border-white/20"
              }`}
            >
              {char}
            </button>
          ))}
        </div>
      )}

      <div className="space-y-1 text-xs">
        {filteredEvents.slice(0, 50).map((event, idx) => (
          <div
            key={idx}
            onClick={() => onEventClick?.(event)}
            className="flex items-start gap-2 py-1 px-2 border border-transparent hover:border-white/10 rounded cursor-pointer transition-all text-white/70 hover:text-white"
          >
            <div className="flex-shrink-0 text-white/40 w-12">
              {(event.timestamp / 1000).toFixed(1)}s
            </div>
            <div className="flex-1">
              <div className="inline-block px-1.5 py-0.5 rounded bg-white/5 text-white/80">
                {event.type}
              </div>{" "}
              <span className="text-white">{event.actor}</span>{" "}
              <span className="text-white/60">{event.action}</span>
              {event.target && (
                <span className="text-white/60">→ {event.target}</span>
              )}
            </div>
            <div
              className="flex-shrink-0 h-1 bg-white/10 rounded w-16"
              style={{
                background: `linear-gradient(90deg, transparent 0%, white 50%, transparent 100%)`,
              }}
            >
              <div
                className="h-full bg-blue-500/40"
                style={{ width: `${(event.timestamp / maxTime) * 100}%` }}
              />
            </div>
          </div>
        ))}
      </div>
      {filteredEvents.length > 50 && (
        <p className="text-xs text-white/40 mt-4">
          Showing 50 of {filteredEvents.length} events
        </p>
      )}
    </Card>
  );
};

Timeline.displayName = "Timeline";
