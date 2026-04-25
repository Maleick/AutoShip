import React, { useMemo, useState } from "react";
import { Card } from "../ui/Card";
import { LogEvent } from "./types";

interface EventListProps {
  events: LogEvent[];
  searchTerm?: string;
  onSearchChange?: (term: string) => void;
  onEventSelect?: (event: LogEvent) => void;
}

export const EventList: React.FC<EventListProps> = ({
  events,
  searchTerm = "",
  onSearchChange,
  onEventSelect,
}) => {
  const [localSearch, setLocalSearch] = useState(searchTerm);

  const filtered = useMemo(() => {
    if (!localSearch) return events;
    const lower = localSearch.toLowerCase();
    return events.filter(
      (e) =>
        e.type.toLowerCase().includes(lower) ||
        e.actor.toLowerCase().includes(lower) ||
        e.action.toLowerCase().includes(lower) ||
        e.target?.toLowerCase().includes(lower),
    );
  }, [events, localSearch]);

  const handleSearch = (term: string) => {
    setLocalSearch(term);
    onSearchChange?.(term);
  };

  return (
    <Card
      title="Event Log"
      subtitle="v1: functional, minimal styling"
      variant="outlined"
    >
      <div className="space-y-3">
        <input
          type="text"
          placeholder="Search events..."
          value={localSearch}
          onChange={(e) => handleSearch(e.target.value)}
          className="w-full px-3 py-2 text-xs bg-white/5 border border-white/10 rounded text-white placeholder-white/40 focus:outline-none focus:border-white/30 transition-all"
        />

        <div className="max-h-96 overflow-y-auto space-y-1">
          {filtered.slice(0, 100).map((event, idx) => (
            <div
              key={idx}
              onClick={() => onEventSelect?.(event)}
              className="flex flex-wrap gap-2 p-2 text-xs border border-transparent hover:border-white/10 rounded cursor-pointer transition-all text-white/70 hover:text-white hover:bg-white/5"
            >
              <span className="text-white/40 flex-shrink-0">
                [{(event.timestamp / 1000).toFixed(1)}s]
              </span>
              <span className="inline-block px-1.5 py-0.5 rounded bg-white/10 text-white/90 flex-shrink-0">
                {event.type}
              </span>
              <span className="text-white font-medium">{event.actor}</span>
              <span className="text-white/60">{event.action}</span>
              {event.target && (
                <>
                  <span className="text-white/40">→</span>
                  <span className="text-white">{event.target}</span>
                </>
              )}
              {event.details && (
                <span className="text-white/40 text-[10px] flex-shrink-0">
                  {JSON.stringify(event.details).substring(0, 50)}
                </span>
              )}
            </div>
          ))}
        </div>

        {filtered.length > 100 && (
          <p className="text-xs text-white/40">
            Showing 100 of {filtered.length} events
          </p>
        )}
        {filtered.length === 0 && (
          <p className="text-xs text-white/40 text-center py-4">
            No events match your search
          </p>
        )}
      </div>
    </Card>
  );
};

EventList.displayName = "EventList";
