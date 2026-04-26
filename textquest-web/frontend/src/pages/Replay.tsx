import { useEffect, useMemo, useRef, useState } from "react";
import {
  Bookmark,
  ChevronLeft,
  ChevronRight,
  Clock3,
  Filter,
  Layers3,
  Pause,
  Play,
  Search,
} from "lucide-react";
import {
  buildBookmark,
  buildReplaySession,
  detectHighlights,
  formatClock,
  resolveReplaySnapshot,
  type ReplayBookmark,
  type ReplayEvent,
  type ReplayMetricCard,
  type ReplayTableRow,
} from "./replay-data.ts";
import { EventList } from "../components/replay/EventList.tsx";
import { MetricCard } from "../components/replay/MetricCard.tsx";

type ReplayView = "tables" | "timeline" | "events";

const VIEW_LABELS: Array<{ id: ReplayView; label: string; hint: string }> = [
  { id: "tables", label: "Tables", hint: "metrics + drilldown" },
  { id: "timeline", label: "Timeline", hint: "swimlanes + filters" },
  { id: "events", label: "Events", hint: "virtualized raw log" },
];

const SPEEDS = [0.25, 0.5, 1, 2, 4];
const TIMELINE_FILTERS = [
  "Smite",
  "Arc Bolt",
  "Heal",
  "Clarity",
  "Root",
  "Dispel",
  "Bane",
  "Ward",
  "Fireburst",
  "Mend",
];
function isInputTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  return target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable;
}

function getSkillTone(category: string): string {
  switch (category) {
    case "Deaths":
    case "Wipes":
      return "border-rose-400/40 bg-rose-400/10 text-rose-200";
    case "Mez breaks":
    case "Close calls":
      return "border-amber-400/40 bg-amber-400/10 text-amber-200";
    case "Named kills":
    case "Full-clears":
      return "border-emerald-400/40 bg-emerald-400/10 text-emerald-200";
    case "Operator bookmarks":
      return "border-neriak-magenta/40 bg-neriak-magenta/10 text-neriak-magenta";
    default:
      return "border-cyan-400/40 bg-cyan-400/10 text-cyan-200";
  }
}

function formatPercent(ts: number, duration: number): string {
  return `${((ts / duration) * 100).toFixed(1)}%`;
}

function kindToGlyph(kind: ReplayEvent["kind"]): string {
  switch (kind) {
    case "damage":
      return "✦";
    case "heal":
      return "+";
    case "cast":
      return "⌁";
    case "buff":
      return "⬤";
    case "debuff":
      return "◌";
    case "death":
      return "✕";
    case "pull":
      return "↯";
    case "resource":
      return "◆";
    case "operator":
      return "⌨";
    case "decision":
      return "◫";
    case "kill":
    case "named_kill":
      return "☠";
    case "mez":
    case "mez_break":
    case "mez_chain":
      return "◎";
    case "full_clear":
      return "✓";
    case "close_call":
      return "!";
    default:
      return "•";
  }
}

function rowsToListRows(rows: ReplayMetricCard["rows"]): ReplayTableRow[] {
  return rows.map((row) => ({
    ...row,
    detail: row.detail,
  }));
}

export function Replay() {
  const session = useMemo(() => buildReplaySession(), []);
  const [view, setView] = useState<ReplayView>("tables");
  const [cursorTs, setCursorTs] = useState(0);
  const [hoverTs, setHoverTs] = useState<number | null>(null);
  const [playing, setPlaying] = useState(false);
  const [speedIndex, setSpeedIndex] = useState(2);
  const [selectedMetricKey, setSelectedMetricKey] = useState(session.metrics[0]?.key ?? "dps");
  const [search, setSearch] = useState("");
  const [selectedKinds, setSelectedKinds] = useState<Set<string>>(
    () => new Set(TIMELINE_FILTERS.slice(0, 6)),
  );
  const [bookmarks, setBookmarks] = useState<ReplayBookmark[]>(() => {
    try {
      const raw = window.localStorage.getItem("textquest.replay.bookmarks");
      if (!raw) {
        return [];
      }

      return JSON.parse(raw) as ReplayBookmark[];
    } catch {
      return [];
    }
  });
  const [seekLatencyMs, setSeekLatencyMs] = useState(0);
  const searchRef = useRef<HTMLInputElement | null>(null);
  const lastTapRef = useRef<Record<string, number>>({});

  const snapshot = useMemo(
    () => resolveReplaySnapshot(session, hoverTs ?? cursorTs),
    [cursorTs, hoverTs, session],
  );
  const activeTs = hoverTs ?? cursorTs;
  const activeEvent = snapshot.event;
  const activeTsRef = useRef(activeTs);
  const snapshotRef = useRef(snapshot);
  const visibleHighlights = useMemo(
    () => detectHighlights(session.events, bookmarks),
    [bookmarks, session.events],
  );

  useEffect(() => {
    activeTsRef.current = activeTs;
    snapshotRef.current = snapshot;
  }, [activeTs, snapshot]);

  useEffect(() => {
    try {
      window.localStorage.setItem("textquest.replay.bookmarks", JSON.stringify(bookmarks));
      window.localStorage.setItem(
        "operator.ndjson.zst",
        bookmarks.map((bookmark) => bookmark.persistedLine).join("\n"),
      );
    } catch {
      // Persisting bookmarks is best-effort in the browser mock.
    }
  }, [bookmarks]);

  useEffect(() => {
    if (!playing) {
      return;
    }

    const interval = window.setInterval(() => {
      setCursorTs((value) => Math.min(session.durationSec, value + SPEEDS[speedIndex] * 0.05));
    }, 50);

    return () => window.clearInterval(interval);
  }, [playing, session.durationSec, speedIndex]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (isInputTarget(event.target)) {
        return;
      }

      const key = event.key;
      if (key === "/") {
        event.preventDefault();
        searchRef.current?.focus();
        return;
      }

      if (key === "k" || key === "K") {
        event.preventDefault();
        setPlaying((current) => !current);
        return;
      }

      if (key === "b" || key === "B") {
        event.preventDefault();
        toggleBookmark();
        return;
      }

      if (
        key === "[" ||
        key === "]" ||
        key === "," ||
        key === "." ||
        key === "j" ||
        key === "J" ||
        key === "l" ||
        key === "L" ||
        key === "<" ||
        key === ">"
      ) {
        event.preventDefault();
      }

      if (key === "[" || key === "]") {
        frameStep(key === "[" ? -1 : 1);
        return;
      }

      if (key === "," || key === ".") {
        skipSeconds(key === "," ? -2 : 2);
        return;
      }

      if (key === "j" || key === "J" || key === "l" || key === "L") {
        const now = performance.now();
        const tapKey = key.toUpperCase();
        const lastTap = lastTapRef.current[tapKey] ?? 0;
        const repeated = now - lastTap < 300;
        lastTapRef.current[tapKey] = now;
        skipSeconds(tapKey === "J" ? (repeated ? -4 : -2) : repeated ? 4 : 2);
        return;
      }

      if (key === "<") {
        event.preventDefault();
        setSpeedIndex((current) => Math.max(0, current - 1));
        return;
      }

      if (key === ">") {
        event.preventDefault();
        setSpeedIndex((current) => Math.min(SPEEDS.length - 1, current + 1));
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [session.durationSec]);

  function seekTo(ts: number) {
    const started = performance.now();
    const nextSnapshot = resolveReplaySnapshot(session, ts);
    setCursorTs(nextSnapshot.ts);
    setHoverTs(null);
    setSeekLatencyMs(performance.now() - started);
  }

  function skipSeconds(delta: number) {
    seekTo(activeTsRef.current + delta);
    setPlaying(false);
  }

  function frameStep(delta: number) {
    const currentFrameIndex = snapshotRef.current.frameIndex;
    const nextIndex = Math.max(
      0,
      Math.min(session.frameTimes.length - 1, currentFrameIndex + delta),
    );
    seekTo(session.frameTimes[nextIndex] ?? activeTsRef.current);
    setPlaying(false);
  }

  function toggleBookmark() {
    const currentTs = activeTsRef.current;
    const bookmark = buildBookmark(currentTs, session.id);
    setBookmarks((current) => {
      const existingIndex = current.findIndex((item) => Math.abs(item.ts - currentTs) <= 0.5);
      if (existingIndex >= 0) {
        const next = current.filter((_, index) => index !== existingIndex);
        return next;
      }

      return [...current, bookmark].sort((a, b) => a.ts - b.ts);
    });
  }

  const filteredEvents = useMemo(() => {
    const selected = selectedKinds.size === 0 ? new Set(session.abilityCatalog) : selectedKinds;
    return session.events.filter((event) => {
      const matchesAbility = !event.ability || selected.has(event.ability);
      if (!matchesAbility) {
        return false;
      }

      if (!search.trim()) {
        return true;
      }

      const haystack = [
        event.actor,
        event.target,
        event.ability,
        event.kind,
        event.detail,
        event.policyVersion,
        formatClock(event.ts),
      ]
        .filter(Boolean)
        .join(" ")
        .toLowerCase();

      return haystack.includes(search.trim().toLowerCase());
    });
  }, [search, selectedKinds, session.abilityCatalog, session.events]);

  const filteredKinds = useMemo(() => {
    return TIMELINE_FILTERS.map((label) => {
      const count = session.events.filter((event) => event.ability === label).length;
      return {
        id: label,
        label,
        active: selectedKinds.has(label),
        count,
      };
    });
  }, [selectedKinds, session.events]);

  const currentMetric =
    session.metrics.find((metric) => metric.key === selectedMetricKey) ?? session.metrics[0];
  const timelineLanes = useMemo(() => {
    return buildTimelineLanes(session, selectedKinds);
  }, [selectedKinds, session]);

  const visibleRows = useMemo<ReplayTableRow[]>(() => {
    if (view !== "events") {
      return [];
    }

    return filteredEvents.map((event) => ({
      id: event.id,
      ts: event.ts,
      kind: event.kind,
      label: event.actor,
      actor: event.actor,
      target: event.target,
      ability: event.ability,
      value:
        event.value ??
        (typeof event.amount === "number" ? event.amount.toLocaleString() : undefined),
      detail: event.detail,
      lane: event.lane,
      policyVersion: event.policyVersion,
    }));
  }, [filteredEvents, view]);

  return (
    <div className="flex h-full flex-col gap-4 p-4">
      <section className="rounded-2xl border border-neriak-dim bg-gradient-to-br from-void via-panel/40 to-elevated/60 p-4 shadow-[0_24px_120px_-70px_rgba(0,0,0,0.85)]">
        <div className="flex flex-col gap-4 xl:flex-row xl:items-center">
          <div className="space-y-2">
            <div className="flex items-center gap-3">
              <span className="rounded-full border border-neriak-magenta/40 bg-neriak-magenta/10 px-3 py-1 font-mono text-[10px] uppercase tracking-[0.22em] text-neriak-magenta">
                session replay
              </span>
              <span className="font-mono text-[10px] uppercase tracking-[0.2em] text-neriak-muted">
                seek {seekLatencyMs.toFixed(2)} ms
              </span>
            </div>
            <h1 className="text-2xl font-semibold text-neriak-text">{session.title}</h1>
            <p className="max-w-3xl text-sm text-neriak-muted">{session.description}</p>
            <div className="flex flex-wrap items-center gap-3 font-mono text-[10px] uppercase tracking-[0.18em] text-neriak-muted">
              <span className="inline-flex items-center gap-1 rounded-full border border-neriak-dim bg-void px-2.5 py-1">
                <Clock3 className="h-3.5 w-3.5" />
                {formatClock(session.durationSec)}
              </span>
              <span className="inline-flex items-center gap-1 rounded-full border border-neriak-dim bg-void px-2.5 py-1">
                <Layers3 className="h-3.5 w-3.5" />
                {session.events.length.toLocaleString()} events
              </span>
              <span className="inline-flex items-center gap-1 rounded-full border border-neriak-dim bg-void px-2.5 py-1">
                <Filter className="h-3.5 w-3.5" />
                {selectedKinds.size || session.abilityCatalog.length} ability filters
              </span>
            </div>
          </div>

          <div className="flex flex-1 flex-wrap items-center justify-end gap-2">
            <button
              onClick={() => skipSeconds(-2)}
              className="inline-flex items-center gap-1.5 rounded-md border border-neriak-dim px-3 py-2 text-xs uppercase tracking-[0.15em] text-neriak-muted hover:border-neriak-magenta hover:text-neriak-magenta"
            >
              <ChevronLeft className="h-4 w-4" />
              2s
            </button>
            <button
              onClick={() => setPlaying((current) => !current)}
              className="inline-flex items-center gap-1.5 rounded-md border border-neriak-magenta/40 bg-neriak-magenta/10 px-3 py-2 text-xs uppercase tracking-[0.15em] text-neriak-magenta"
            >
              {playing ? <Pause className="h-4 w-4" /> : <Play className="h-4 w-4" />}
              {playing ? "pause" : "play"}
            </button>
            <button
              onClick={() => skipSeconds(2)}
              className="inline-flex items-center gap-1.5 rounded-md border border-neriak-dim px-3 py-2 text-xs uppercase tracking-[0.15em] text-neriak-muted hover:border-neriak-magenta hover:text-neriak-magenta"
            >
              2s
              <ChevronRight className="h-4 w-4" />
            </button>

            <div className="ml-0 flex items-center gap-1 rounded-md border border-neriak-dim bg-void p-1">
              {SPEEDS.map((speed, index) => (
                <button
                  key={speed}
                  onClick={() => setSpeedIndex(index)}
                  className={`rounded px-2.5 py-1 font-mono text-[10px] uppercase tracking-[0.16em] ${
                    index === speedIndex
                      ? "bg-neriak-magenta text-void"
                      : "text-neriak-muted hover:text-neriak-text"
                  }`}
                >
                  {speed}x
                </button>
              ))}
            </div>

            <button
              onClick={toggleBookmark}
              className="inline-flex items-center gap-1.5 rounded-md border border-neriak-dim px-3 py-2 text-xs uppercase tracking-[0.15em] text-neriak-muted hover:border-neriak-magenta hover:text-neriak-magenta"
            >
              <Bookmark className="h-4 w-4" />
              bookmark
            </button>

            <label className="inline-flex items-center gap-2 rounded-md border border-neriak-dim bg-void px-3 py-2 text-neriak-muted">
              <Search className="h-4 w-4" />
              <input
                ref={searchRef}
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                className="w-56 bg-transparent text-sm text-neriak-text outline-none placeholder:text-neriak-dim"
                placeholder="Search events"
              />
            </label>
          </div>
        </div>

        <div className="mt-4 flex flex-wrap gap-2">
          {VIEW_LABELS.map((item) => {
            const active = view === item.id;
            return (
              <button
                key={item.id}
                onClick={() => setView(item.id)}
                className={`rounded-full border px-4 py-2 text-left transition-colors ${
                  active
                    ? "border-neriak-magenta bg-neriak-magenta/10 text-neriak-magenta"
                    : "border-neriak-dim bg-void text-neriak-muted hover:border-neriak-magenta hover:text-neriak-text"
                }`}
              >
                <div className="font-mono text-[10px] uppercase tracking-[0.2em]">{item.label}</div>
                <div className="text-xs normal-case tracking-normal">{item.hint}</div>
              </button>
            );
          })}
        </div>
      </section>

      <div className="grid min-h-0 flex-1 gap-4 xl:grid-cols-[minmax(0,1fr)_340px]">
        <section className="min-h-0 space-y-4">
          {view === "tables" && (
            <TablesView
              metrics={session.metrics}
              activeTs={activeTs}
              selectedMetric={currentMetric}
              onSelectMetric={(key) => setSelectedMetricKey(key)}
              onJumpToTs={seekTo}
              onHoverTs={setHoverTs}
            />
          )}

          {view === "timeline" && (
            <TimelineView
              session={session}
              lanes={timelineLanes}
              activeTs={activeTs}
              selectedKinds={selectedKinds}
              filters={filteredKinds}
              onToggleKind={(ability) => {
                setSelectedKinds((current) => {
                  const next = new Set(current);
                  if (next.has(ability)) {
                    next.delete(ability);
                  } else {
                    next.add(ability);
                  }
                  return next;
                });
              }}
              onJumpToTs={seekTo}
              onHoverTs={setHoverTs}
            />
          )}

          {view === "events" && (
            <EventList
              title="Raw events"
              rows={visibleRows}
              columns={["ts", "kind", "actor", "target", "ability", "detail", "policyVersion"]}
              search={search}
              onSearchChange={setSearch}
              filters={filteredKinds}
              onToggleFilter={(ability) => {
                setSelectedKinds((current) => {
                  const next = new Set(current);
                  if (next.has(ability)) {
                    next.delete(ability);
                  } else {
                    next.add(ability);
                  }
                  return next;
                });
              }}
              activeTs={activeTs}
              onHoverTs={setHoverTs}
              onJumpToTs={seekTo}
              height={620}
              rowHeight={48}
            />
          )}
        </section>

        <aside className="min-h-0 space-y-4">
          <section className="rounded-xl border border-neriak-dim bg-panel/80 p-4">
            <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-neriak-muted">
              Shared cursor
            </p>
            <p className="mt-2 text-2xl font-semibold text-neriak-text">{formatClock(activeTs)}</p>
            <p className="mt-1 text-sm text-neriak-muted">{activeEvent.detail}</p>
            <div className="mt-3 grid grid-cols-2 gap-2 text-[10px] uppercase tracking-[0.18em] text-neriak-muted">
              <span className="rounded-md border border-neriak-dim bg-void px-2 py-1">
                {activeEvent.kind}
              </span>
              <span className="rounded-md border border-neriak-dim bg-void px-2 py-1">
                frame {snapshot.frameIndex.toLocaleString()}
              </span>
              <span className="rounded-md border border-neriak-dim bg-void px-2 py-1">
                keyframe {formatClock(snapshot.keyframeTs)}
              </span>
              <span className="rounded-md border border-neriak-dim bg-void px-2 py-1">
                event {snapshot.eventIndex.toLocaleString()}
              </span>
            </div>
          </section>

          <section className="rounded-xl border border-neriak-dim bg-panel/80 p-4">
            <div className="flex items-center justify-between gap-3">
              <div>
                <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-neriak-muted">
                  Auto-highlights
                </p>
                <p className="text-sm text-neriak-text">
                  {visibleHighlights.length.toLocaleString()} detected cues
                </p>
              </div>
              <span className="rounded-full border border-neriak-dim bg-void px-2 py-1 font-mono text-[10px] uppercase tracking-[0.18em] text-neriak-muted">
                labeled fixture
              </span>
            </div>
            <div className="mt-3 space-y-2">
              {visibleHighlights.slice(0, 10).map((highlight) => (
                <button
                  key={highlight.id}
                  onClick={() => seekTo(highlight.ts)}
                  className={`w-full rounded-lg border px-3 py-2 text-left transition-colors ${getSkillTone(highlight.category)}`}
                >
                  <div className="flex items-center justify-between gap-3">
                    <span className="font-mono text-[10px] uppercase tracking-[0.18em]">
                      {formatClock(highlight.ts)}
                    </span>
                    <span className="text-xs uppercase tracking-[0.18em]">
                      {highlight.category}
                    </span>
                  </div>
                  <p className="mt-1 text-sm font-medium text-neriak-text">{highlight.title}</p>
                  <p className="mt-1 text-xs text-neriak-muted">{highlight.detail}</p>
                </button>
              ))}
            </div>
          </section>

          <section className="rounded-xl border border-neriak-dim bg-panel/80 p-4">
            <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-neriak-muted">
              Bookmarks
            </p>
            <p className="text-sm text-neriak-text">
              {bookmarks.length.toLocaleString()} saved markers
            </p>
            <div className="mt-3 space-y-2">
              {bookmarks.length === 0 && (
                <p className="text-sm text-neriak-dim">Press B to pin the current frame.</p>
              )}
              {bookmarks.map((bookmark) => (
                <button
                  key={bookmark.id}
                  onClick={() => seekTo(bookmark.ts)}
                  className="w-full rounded-lg border border-neriak-dim bg-void px-3 py-2 text-left transition-colors hover:border-neriak-magenta hover:bg-neriak-magenta/10"
                >
                  <div className="flex items-center justify-between gap-3">
                    <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-neriak-muted">
                      {formatClock(bookmark.ts)}
                    </span>
                    <span className="text-xs uppercase tracking-[0.18em] text-neriak-magenta">
                      operator
                    </span>
                  </div>
                  <p className="mt-1 text-sm text-neriak-text">{bookmark.label}</p>
                  <p className="mt-1 text-xs text-neriak-muted">{bookmark.persistedLine}</p>
                </button>
              ))}
            </div>
          </section>

          <section className="rounded-xl border border-neriak-dim bg-panel/80 p-4">
            <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-neriak-muted">
              Storage seek
            </p>
            <p className="mt-2 text-sm text-neriak-text">
              meta.json + dictionary -&gt; keyframe -&gt; delta replay -&gt; render
            </p>
            <div className="mt-3 space-y-2 text-xs text-neriak-muted">
              <div className="rounded-md border border-neriak-dim bg-void px-3 py-2">
                1. locate nearest keyframe {formatClock(snapshot.keyframeTs)}
              </div>
              <div className="rounded-md border border-neriak-dim bg-void px-3 py-2">
                2. replay forward to {formatClock(snapshot.ts)}
              </div>
              <div className="rounded-md border border-neriak-dim bg-void px-3 py-2">
                3. render snapshot in {seekLatencyMs.toFixed(2)} ms
              </div>
            </div>
          </section>
        </aside>
      </div>
    </div>
  );
}

function TablesView({
  metrics,
  activeTs,
  selectedMetric,
  onSelectMetric,
  onJumpToTs,
  onHoverTs,
}: {
  metrics: ReplayMetricCard[];
  activeTs: number;
  selectedMetric: ReplayMetricCard;
  onSelectMetric: (key: string) => void;
  onJumpToTs: (ts: number) => void;
  onHoverTs: (ts: number | null) => void;
}) {
  return (
    <div className="grid min-h-0 gap-4 xl:grid-cols-[minmax(0,1.2fr)_minmax(0,0.8fr)]">
      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-2">
        {metrics.map((metric) => (
          <MetricCard
            key={metric.key}
            metric={metric}
            selected={metric.key === selectedMetric.key}
            activeTs={activeTs}
            onSelect={() => onSelectMetric(metric.key)}
            onJumpToTs={onJumpToTs}
          />
        ))}
      </div>

      <EventList
        title={`Metric drilldown: ${selectedMetric.label}`}
        rows={rowsToListRows(selectedMetric.rows)}
        columns={["ts", "label", "value", "detail"]}
        activeTs={activeTs}
        onHoverTs={onHoverTs}
        onJumpToTs={onJumpToTs}
        height={560}
        rowHeight={58}
        compact
      />
    </div>
  );
}

function TimelineView({
  session,
  lanes,
  activeTs,
  selectedKinds,
  filters,
  onToggleKind,
  onJumpToTs,
  onHoverTs,
}: {
  session: ReturnType<typeof buildReplaySession>;
  lanes: Array<{
    id: string;
    label: string;
    kind: "pc" | "npc" | "operator" | "decision";
    events: ReplayEvent[];
  }>;
  activeTs: number;
  selectedKinds: Set<string>;
  filters: Array<{ id: string; label: string; active: boolean; count: number }>;
  onToggleKind: (ability: string) => void;
  onJumpToTs: (ts: number) => void;
  onHoverTs: (ts: number | null) => void;
}) {
  return (
    <section className="rounded-xl border border-neriak-dim bg-panel/80 p-4">
      <div className="flex flex-wrap gap-2">
        {filters.map((filter) => (
          <button
            key={filter.id}
            onClick={() => onToggleKind(filter.id)}
            className={`rounded-full border px-3 py-1 font-mono text-[10px] uppercase tracking-[0.18em] transition-colors ${
              filter.active
                ? "border-neriak-magenta bg-neriak-magenta/10 text-neriak-magenta"
                : "border-neriak-dim text-neriak-muted hover:border-neriak-magenta hover:text-neriak-text"
            }`}
          >
            {filter.label}
            <span className="ml-2 text-[9px] text-neriak-dim">{filter.count}</span>
          </button>
        ))}
      </div>

      <div className="mt-4 space-y-3">
        {lanes.map((lane) => (
          <TimelineLane
            key={lane.id}
            lane={lane}
            session={session}
            activeTs={activeTs}
            selectedKinds={selectedKinds}
            onHoverTs={onHoverTs}
            onJumpToTs={onJumpToTs}
          />
        ))}
      </div>
    </section>
  );
}

function TimelineLane({
  lane,
  session,
  activeTs,
  selectedKinds,
  onHoverTs,
  onJumpToTs,
}: {
  lane: {
    id: string;
    label: string;
    kind: "pc" | "npc" | "operator" | "decision";
    events: ReplayEvent[];
  };
  session: ReturnType<typeof buildReplaySession>;
  activeTs: number;
  selectedKinds: Set<string>;
  onHoverTs: (ts: number | null) => void;
  onJumpToTs: (ts: number) => void;
}) {
  const laneEvents =
    lane.kind === "pc" || lane.kind === "npc"
      ? lane.events.filter(
          (event) => !event.ability || selectedKinds.size === 0 || selectedKinds.has(event.ability),
        )
      : lane.events;

  return (
    <div className="rounded-lg border border-neriak-dim bg-void/80 p-3">
      <div className="flex items-center gap-3">
        <span className="min-w-36 font-mono text-[10px] uppercase tracking-[0.22em] text-neriak-muted">
          {lane.label}
        </span>
        <div className="relative h-12 flex-1 overflow-hidden rounded-md border border-neriak-dim bg-panel">
          <div
            className="absolute inset-y-0 left-0 w-px bg-neriak-magenta"
            style={{ left: `${formatPercent(activeTs, session.durationSec)}` }}
          />
          {laneEvents.map((event) => {
            const position = `${formatPercent(event.ts, session.durationSec)}`;
            const isActive = Math.abs(event.ts - activeTs) <= 0.5;
            return (
              <button
                key={event.id}
                title={`${formatClock(event.ts)} - ${event.detail}`}
                onMouseEnter={() => onHoverTs(event.ts)}
                onMouseLeave={() => onHoverTs(null)}
                onClick={() => onJumpToTs(event.ts)}
                className={`absolute top-1/2 -translate-y-1/2 rounded-full border px-2 py-1 font-mono text-[10px] uppercase tracking-[0.18em] transition-transform ${
                  isActive
                    ? "border-neriak-magenta bg-neriak-magenta text-void shadow-[0_0_20px_-8px_var(--color-neriak-magenta)]"
                    : "border-neriak-dim bg-void text-neriak-muted hover:border-neriak-magenta hover:text-neriak-magenta"
                }`}
                style={{ left: position }}
              >
                {kindToGlyph(event.kind)}
              </button>
            );
          })}
        </div>
      </div>

      <div className="mt-2 grid grid-cols-[10rem_minmax(0,1fr)] gap-3 text-xs text-neriak-muted">
        <span>cursor</span>
        <span className="text-neriak-text">{formatClock(activeTs)}</span>
      </div>
    </div>
  );
}

function buildTimelineLanes(
  session: ReturnType<typeof buildReplaySession>,
  selectedKinds: Set<string>,
) {
  const pcEvents = session.events.filter(
    (event) =>
      event.lane === "pc" &&
      (!event.ability || selectedKinds.size === 0 || selectedKinds.has(event.ability)),
  );
  const npcEvents = session.events.filter(
    (event) =>
      event.lane === "npc" &&
      (!event.ability || selectedKinds.size === 0 || selectedKinds.has(event.ability)),
  );
  const operatorEvents = session.events.filter(
    (event) =>
      event.lane === "operator" &&
      (!event.ability || selectedKinds.size === 0 || selectedKinds.has(event.ability)),
  );
  const decisionEvents = session.events.filter(
    (event) =>
      event.lane === "decision" &&
      (!event.ability || selectedKinds.size === 0 || selectedKinds.has(event.ability)),
  );

  const pcLanes = ["Astra", "Bren", "Cora", "Dain"].map((label) => ({
    id: `pc-${label}`,
    label,
    kind: "pc" as const,
    events: pcEvents.filter((event) => event.actor === label || event.target === label),
  }));

  return [
    ...pcLanes,
    {
      id: "npc-encounter",
      label: "Engaged-NPC",
      kind: "npc" as const,
      events: npcEvents,
    },
    {
      id: "operator-input",
      label: "Operator input",
      kind: "operator" as const,
      events: operatorEvents,
    },
    {
      id: "orchestrator-decision",
      label: "Orchestrator decision",
      kind: "decision" as const,
      events: decisionEvents,
    },
  ];
}
