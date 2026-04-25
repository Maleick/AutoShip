import { useMemo, useRef, useState } from "react";
import { Search } from "lucide-react";
import { formatClock } from "../../pages/replay-data.ts";
import type { ReplayTableRow } from "../../pages/replay-data.ts";

export type ReplayListColumn =
  | "ts"
  | "kind"
  | "label"
  | "actor"
  | "target"
  | "ability"
  | "value"
  | "detail"
  | "lane"
  | "policyVersion";

interface FilterChip {
  id: string;
  label: string;
  active: boolean;
  count?: number;
}

interface EventListProps {
  rows: ReplayTableRow[];
  columns: ReplayListColumn[];
  title: string;
  search?: string;
  onSearchChange?: (value: string) => void;
  filters?: FilterChip[];
  onToggleFilter?: (id: string) => void;
  activeTs: number;
  onHoverTs?: (ts: number | null) => void;
  onJumpToTs: (ts: number) => void;
  height?: number;
  rowHeight?: number;
  compact?: boolean;
}

const HEADER_LABELS: Record<ReplayListColumn, string> = {
  ts: "Time",
  kind: "Kind",
  label: "Label",
  actor: "Actor",
  target: "Target",
  ability: "Ability",
  value: "Value",
  detail: "Detail",
  lane: "Lane",
  policyVersion: "Policy",
};

function rowMatchesSearch(row: ReplayTableRow, search: string): boolean {
  if (!search.trim()) {
    return true;
  }

  const haystack = [
    row.label,
    row.kind,
    row.actor,
    row.target,
    row.ability,
    row.value,
    row.detail,
    row.lane,
    row.policyVersion,
    formatClock(row.ts),
  ]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();

  return haystack.includes(search.trim().toLowerCase());
}

export function EventList({
  rows,
  columns,
  title,
  search = "",
  onSearchChange,
  filters,
  onToggleFilter,
  activeTs,
  onHoverTs,
  onJumpToTs,
  height = 480,
  rowHeight = 44,
  compact = false,
}: EventListProps) {
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const viewportHeight = height;
  const rowSpan = compact ? Math.max(36, rowHeight - 6) : rowHeight;

  const filteredRows = useMemo(
    () => rows.filter((row) => rowMatchesSearch(row, search)),
    [rows, search],
  );

  const totalHeight = filteredRows.length * rowSpan;
  const start = Math.max(0, Math.floor(scrollTop / rowSpan) - 6);
  const end = Math.min(filteredRows.length, Math.ceil((scrollTop + viewportHeight) / rowSpan) + 6);
  const visibleRows = filteredRows.slice(start, end);

  return (
    <section className="rounded-xl border border-neriak-dim bg-panel/80 overflow-hidden">
      <div className="flex flex-wrap items-center gap-3 border-b border-neriak-dim px-4 py-3">
        <div>
          <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-neriak-muted">
            {title}
          </p>
          <p className="text-sm text-neriak-text">
            {filteredRows.length.toLocaleString()} visible rows
          </p>
        </div>

        {onSearchChange && (
          <label className="ml-auto flex items-center gap-2 rounded-md border border-neriak-dim bg-void px-3 py-2 text-neriak-muted">
            <Search className="h-4 w-4" strokeWidth={1.8} />
            <input
              value={search}
              onChange={(event) => onSearchChange(event.target.value)}
              className="w-56 bg-transparent text-sm text-neriak-text outline-none placeholder:text-neriak-dim"
              placeholder="Search events"
            />
          </label>
        )}
      </div>

      {filters && filters.length > 0 && (
        <div className="flex flex-wrap gap-2 border-b border-neriak-dim px-4 py-3">
          {filters.map((filter) => (
            <button
              key={filter.id}
              onClick={() => onToggleFilter?.(filter.id)}
              className={`rounded-full border px-3 py-1 font-mono text-[10px] uppercase tracking-[0.18em] transition-colors ${
                filter.active
                  ? "border-neriak-magenta bg-neriak-magenta/10 text-neriak-magenta"
                  : "border-neriak-dim text-neriak-muted hover:border-neriak-magenta/50 hover:text-neriak-text"
              }`}
            >
              {filter.label}
              {typeof filter.count === "number" && (
                <span className="ml-2 text-[9px] text-neriak-dim">{filter.count}</span>
              )}
            </button>
          ))}
        </div>
      )}

      <div
        ref={scrollRef}
        onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
        className="overflow-y-auto"
        style={{ height: viewportHeight }}
      >
        <div className="relative" style={{ height: totalHeight }}>
          <table className="absolute inset-x-0 top-0 w-full border-separate border-spacing-0">
            <thead className="sticky top-0 z-10">
              <tr className="bg-void/95 backdrop-blur">
                {columns.map((column) => (
                  <th
                    key={column}
                    className="border-b border-neriak-dim px-3 py-2 text-left font-mono text-[10px] uppercase tracking-[0.2em] text-neriak-muted"
                  >
                    {HEADER_LABELS[column]}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {visibleRows.map((row, visibleIndex) => {
                const active = Math.abs(row.ts - activeTs) <= 0.5;
                const top = (start + visibleIndex) * rowSpan;
                return (
                  <tr
                    key={row.id}
                    onClick={() => onJumpToTs(row.ts)}
                    onMouseEnter={() => onHoverTs?.(row.ts)}
                    onMouseLeave={() => onHoverTs?.(null)}
                    className={`absolute left-0 right-0 cursor-pointer border-b border-neriak-dim/60 transition-colors ${
                      active ? "bg-neriak-magenta/10" : "hover:bg-elevated/75"
                    }`}
                    style={{ top, height: rowSpan }}
                  >
                    {columns.map((column, columnIndex) => {
                      const isFirst = columnIndex === 0;
                      const value =
                        column === "ts"
                          ? formatClock(row.ts)
                          : column === "label"
                            ? (row.label ?? row.actor ?? row.kind ?? "event")
                            : column === "policyVersion"
                              ? (row.policyVersion ?? "—")
                              : (row[column] ?? "—");

                      return (
                        <td
                          key={`${row.id}-${column}`}
                          className={`px-3 py-2 align-top text-sm ${
                            isFirst ? "pl-4" : ""
                          } ${column === "ts" ? "font-mono text-[10px] uppercase tracking-[0.18em] text-neriak-muted" : "text-neriak-text"}`}
                        >
                          <span
                            className={
                              column === "kind" && row.kind
                                ? "rounded-full border border-neriak-dim bg-void px-2 py-0.5 font-mono text-[10px] uppercase tracking-[0.18em] text-neriak-muted"
                                : ""
                            }
                          >
                            {value as string}
                          </span>
                        </td>
                      );
                    })}
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    </section>
  );
}
