export type ReplayKind =
  | "damage"
  | "heal"
  | "cast"
  | "buff"
  | "debuff"
  | "death"
  | "pull"
  | "resource"
  | "operator"
  | "decision"
  | "kill"
  | "mez"
  | "mez_break"
  | "named_kill"
  | "wipe"
  | "full_clear"
  | "close_call"
  | "mez_chain";

export type ReplayTone = "magenta" | "violet" | "teal" | "amber" | "rose" | "cyan";

export interface ReplayEvent {
  id: string;
  ts: number;
  kind: ReplayKind;
  lane: "pc" | "npc" | "operator" | "decision";
  actor: string;
  target?: string;
  ability?: string;
  detail: string;
  value?: string;
  amount?: number;
  hpPct?: number;
  named?: boolean;
  policyVersion?: string;
}

export interface ReplayTableRow {
  id: string;
  ts: number;
  kind?: string;
  label?: string;
  actor?: string;
  target?: string;
  ability?: string;
  value?: string;
  detail: string;
  lane?: string;
  policyVersion?: string;
}

export interface ReplayMetricCard {
  key: string;
  label: string;
  value: string;
  delta: string;
  tone: ReplayTone;
  rows: ReplayTableRow[];
}

export interface ReplayHighlight {
  id: string;
  category: string;
  ts: number;
  endTs?: number;
  title: string;
  detail: string;
  severity: "info" | "warn" | "critical";
}

export interface ReplayBookmark {
  id: string;
  ts: number;
  label: string;
  createdAt: number;
  persistedLine: string;
}

export interface ReplaySession {
  id: string;
  title: string;
  description: string;
  durationSec: number;
  frameTimes: number[];
  keyframes: number[];
  events: ReplayEvent[];
  metrics: ReplayMetricCard[];
  abilityCatalog: string[];
  policyVersions: Record<string, string>;
}

const PC_NAMES = ["Astra", "Bren", "Cora", "Dain"];
const NPC_NAME = "Ancient Warden";
const ABILITIES = [
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

function pad2(value: number): string {
  return String(value).padStart(2, "0");
}

export function formatClock(seconds: number): string {
  const safeSeconds = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(safeSeconds / 3600);
  const minutes = Math.floor((safeSeconds % 3600) / 60);
  const secs = safeSeconds % 60;
  return `${pad2(hours)}:${pad2(minutes)}:${pad2(secs)}`;
}

function clampTs(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function upperBound(sorted: number[], target: number): number {
  let low = 0;
  let high = sorted.length;
  while (low < high) {
    const mid = Math.floor((low + high) / 2);
    if (sorted[mid] <= target) {
      low = mid + 1;
    } else {
      high = mid;
    }
  }
  return low;
}

function sumMetric(events: ReplayEvent[], kind: ReplayKind): number {
  return events
    .filter((event) => event.kind === kind)
    .reduce(
      (total, event) => total + (event.amount ?? Number(event.value?.replace(/[^0-9.]/g, "") ?? 0)),
      0,
    );
}

function buildRows(events: ReplayEvent[], kind: ReplayKind, labelPrefix: string): ReplayTableRow[] {
  return events
    .filter((event) => event.kind === kind)
    .slice()
    .sort((a, b) => (b.amount ?? 0) - (a.amount ?? 0) || a.ts - b.ts)
    .slice(0, 5)
    .map((event) => ({
      id: event.id,
      ts: event.ts,
      kind: event.kind,
      label: `${labelPrefix} ${event.actor}`,
      actor: event.actor,
      target: event.target,
      ability: event.ability,
      value: event.amount !== undefined ? `${event.amount.toLocaleString()} dmg` : event.value,
      detail: event.detail,
      lane: event.lane,
    }));
}

function makeEvent(index: number, event: Omit<ReplayEvent, "id">): ReplayEvent {
  return {
    id: `re-${String(index).padStart(5, "0")}`,
    ...event,
  };
}

function generateBaseEvents(durationSec: number): ReplayEvent[] {
  const events: ReplayEvent[] = [];
  let index = 0;

  for (let minute = 0; minute < durationSec / 60; minute += 1) {
    const base = minute * 60;
    const pc = PC_NAMES[minute % PC_NAMES.length];
    const support = PC_NAMES[(minute + 1) % PC_NAMES.length];
    const healer = PC_NAMES[(minute + 2) % PC_NAMES.length];
    const ability = ABILITIES[minute % ABILITIES.length];

    events.push(
      makeEvent(index++, {
        ts: base + 2,
        kind: "cast",
        lane: "pc",
        actor: pc,
        target: NPC_NAME,
        ability,
        detail: `${pc} casts ${ability} into ${NPC_NAME}.`,
      }),
    );

    if (minute % 2 === 0) {
      const amount = 1800 + (minute % 7) * 220;
      events.push(
        makeEvent(index++, {
          ts: base + 10,
          kind: "damage",
          lane: "pc",
          actor: pc,
          target: NPC_NAME,
          ability: "Smite",
          amount,
          value: `${amount.toLocaleString()} dmg`,
          detail: `${pc} lands ${amount.toLocaleString()} damage on ${NPC_NAME}.`,
        }),
      );
    }

    if (minute % 3 === 0) {
      const healAmount = 1200 + (minute % 6) * 150;
      events.push(
        makeEvent(index++, {
          ts: base + 16,
          kind: "heal",
          lane: "pc",
          actor: healer,
          target: support,
          ability: "Heal",
          amount: healAmount,
          value: `${healAmount.toLocaleString()} hp`,
          detail: `${healer} restores ${healAmount.toLocaleString()} HP to ${support}.`,
        }),
      );
    }

    if (minute % 4 === 0) {
      events.push(
        makeEvent(index++, {
          ts: base + 24,
          kind: "buff",
          lane: "pc",
          actor: pc,
          target: pc,
          ability: "Clarity",
          detail: `${pc} gains Clarity.`,
        }),
      );
    }

    if (minute % 5 === 0) {
      events.push(
        makeEvent(index++, {
          ts: base + 31,
          kind: "debuff",
          lane: "npc",
          actor: NPC_NAME,
          target: pc,
          ability: "Venom",
          detail: `${pc} is afflicted by Venom.`,
        }),
      );
    }

    if (minute % 6 === 0) {
      const manaPct = Math.max(10, 96 - (minute % 70));
      events.push(
        makeEvent(index++, {
          ts: base + 34,
          kind: "resource",
          lane: "operator",
          actor: "Operator",
          ability: "Mana",
          value: `${manaPct}%`,
          detail: `Operator checks mana at ${manaPct}%.`,
        }),
      );
    }

    if (minute % 8 === 0) {
      const key = ["J", "K", "L", "[", "]", ",", ".", "B"][minute % 8];
      events.push(
        makeEvent(index++, {
          ts: base + 38,
          kind: "operator",
          lane: "operator",
          actor: "Operator",
          ability: key,
          detail: `Operator input ${key} recorded for replay.`,
        }),
      );
    }

    if (minute % 10 === 0) {
      const policyVersion = `v2.${4 + (minute % 5)}.${minute % 3}`;
      events.push(
        makeEvent(index++, {
          ts: base + 42,
          kind: "decision",
          lane: "decision",
          actor: "Orchestrator",
          ability: policyVersion,
          policyVersion,
          detail: `Policy ${policyVersion} selects ${minute % 2 === 0 ? "hold" : "push"} posture.`,
        }),
      );
    }

    if (minute % 15 === 0) {
      events.push(
        makeEvent(index++, {
          ts: base + 48,
          kind: "pull",
          lane: "npc",
          actor: NPC_NAME,
          target: pc,
          ability: "Pull",
          detail: `${NPC_NAME} is engaged by ${pc}.`,
        }),
      );
    }
  }

  // Labeled fixture moments to exercise the auto-highlight rules.
  const wipeTs = 3900;
  PC_NAMES.forEach((pc, i) => {
    events.push(
      makeEvent(index++, {
        ts: wipeTs + i * 6,
        kind: "death",
        lane: "pc",
        actor: pc,
        target: pc,
        ability: "Death",
        detail: `${pc} falls during the wipe window.`,
      }),
    );
  });
  events.push(
    makeEvent(index++, {
      ts: wipeTs + 28,
      kind: "wipe",
      lane: "decision",
      actor: "Encounter Monitor",
      ability: "Reset",
      detail: "Party-wide death burst detected within 30s.",
    }),
  );

  events.push(
    makeEvent(index++, {
      ts: 4320,
      kind: "named_kill",
      lane: "npc",
      actor: NPC_NAME,
      target: "The Echo of Rallos",
      ability: "Named Kill",
      named: true,
      detail: "Named target Echo of Rallos is defeated.",
    }),
  );

  events.push(
    makeEvent(index++, {
      ts: 7200,
      kind: "full_clear",
      lane: "npc",
      actor: "Camp Wave 1",
      target: "Camp Wave 7",
      ability: "Clear",
      detail: "Camp wave 1 reaches last-wave clear without resets.",
    }),
  );

  events.push(
    makeEvent(index++, {
      ts: 9360,
      kind: "close_call",
      lane: "pc",
      actor: "Bren",
      target: "Bren",
      ability: "Recovery",
      hpPct: 8,
      detail: "Bren drops below 10% HP and recovers without a death.",
    }),
  );

  events.push(
    makeEvent(index++, {
      ts: 10440,
      kind: "mez_chain",
      lane: "npc",
      actor: "Shambling Husk",
      target: "Bren",
      ability: "Mesmerize",
      detail: "Long mez chain holds a single mob for 36s.",
    }),
  );

  events.push(
    makeEvent(index++, {
      ts: 10456,
      kind: "mez_break",
      lane: "npc",
      actor: "Shambling Husk",
      target: "Cora",
      ability: "Break Mez",
      detail: "Crowd control breaks early on Shambling Husk.",
    }),
  );

  events.sort((a, b) => a.ts - b.ts || a.id.localeCompare(b.id));
  return events;
}

function buildMetrics(events: ReplayEvent[]): ReplayMetricCard[] {
  const totalDamage = sumMetric(events, "damage");
  const totalHealing = sumMetric(events, "heal");
  const totalCasts = events.filter((event) => event.kind === "cast").length;
  const totalBuffs = events.filter((event) => event.kind === "buff").length;
  const totalDebuffs = events.filter((event) => event.kind === "debuff").length;
  const totalDeaths = events.filter((event) => event.kind === "death").length;
  const totalPulls = events.filter((event) => event.kind === "pull").length;
  const totalResourceChecks = events.filter((event) => event.kind === "resource").length;

  return [
    {
      key: "dps",
      label: "DPS",
      value: totalDamage.toLocaleString(),
      delta: "+18% vs prev pull",
      tone: "magenta",
      rows: buildRows(events, "damage", "Damage"),
    },
    {
      key: "healing",
      label: "Healing",
      value: totalHealing.toLocaleString(),
      delta: "+12% vs baseline",
      tone: "teal",
      rows: buildRows(events, "heal", "Heal"),
    },
    {
      key: "casts",
      label: "Casts",
      value: totalCasts.toLocaleString(),
      delta: "steady",
      tone: "violet",
      rows: buildRows(events, "cast", "Cast"),
    },
    {
      key: "buffs",
      label: "Buffs",
      value: totalBuffs.toLocaleString(),
      delta: "maintenance",
      tone: "amber",
      rows: buildRows(events, "buff", "Buff"),
    },
    {
      key: "debuffs",
      label: "Debuffs",
      value: totalDebuffs.toLocaleString(),
      delta: "watch list",
      tone: "rose",
      rows: buildRows(events, "debuff", "Debuff"),
    },
    {
      key: "deaths",
      label: "Deaths",
      value: totalDeaths.toLocaleString(),
      delta: "needs review",
      tone: "rose",
      rows: buildRows(events, "death", "Death"),
    },
    {
      key: "pulls",
      label: "Pulls",
      value: totalPulls.toLocaleString(),
      delta: "session cadence",
      tone: "cyan",
      rows: buildRows(events, "pull", "Pull"),
    },
    {
      key: "resources",
      label: "Resources",
      value: totalResourceChecks.toLocaleString(),
      delta: "operator watch",
      tone: "teal",
      rows: buildRows(events, "resource", "Check"),
    },
  ];
}

export function buildReplaySession(): ReplaySession {
  const durationSec = 4 * 60 * 60;
  const frameTimes = Array.from({ length: durationSec * 2 + 1 }, (_, index) => index / 2);
  const keyframes = Array.from(
    { length: Math.floor(durationSec / 30) + 1 },
    (_, index) => index * 30,
  );
  const events = generateBaseEvents(durationSec);
  const metrics = buildMetrics(events);

  return {
    id: "recorded-session-4h",
    title: "Recorded replay session",
    description:
      "Four-hour session fixture with shared cursor, bookmarks, and auto-highlight signals.",
    durationSec,
    frameTimes,
    keyframes,
    events,
    metrics,
    abilityCatalog: Array.from(
      new Set(events.filter((event) => event.ability).map((event) => event.ability as string)),
    ).sort(),
    policyVersions: {
      "v2.4.0": "baseline",
      "v2.5.1": "updated policy",
      "v2.6.2": "late-session branch",
    },
  };
}

export function findFrameIndex(frameTimes: number[], ts: number): number {
  const clamped = clampTs(ts, 0, frameTimes[frameTimes.length - 1] ?? 0);
  let low = 0;
  let high = frameTimes.length - 1;
  while (low < high) {
    const mid = Math.ceil((low + high) / 2);
    if (frameTimes[mid] <= clamped) {
      low = mid;
    } else {
      high = mid - 1;
    }
  }
  return low;
}

export function findEventIndex(events: ReplayEvent[], ts: number): number {
  const clamped = clampTs(ts, 0, events[events.length - 1]?.ts ?? 0);
  let low = 0;
  let high = events.length - 1;
  while (low < high) {
    const mid = Math.ceil((low + high) / 2);
    if (events[mid].ts <= clamped) {
      low = mid;
    } else {
      high = mid - 1;
    }
  }
  return low;
}

export interface ReplaySnapshot {
  ts: number;
  frameIndex: number;
  frameTs: number;
  keyframeIndex: number;
  keyframeTs: number;
  eventIndex: number;
  event: ReplayEvent;
  nearbyEvents: ReplayEvent[];
}

export function resolveReplaySnapshot(session: ReplaySession, ts: number): ReplaySnapshot {
  const safeTs = clampTs(ts, 0, session.durationSec);
  const frameIndex = findFrameIndex(session.frameTimes, safeTs);
  const eventIndex = findEventIndex(session.events, safeTs);
  const keyframeIndex = Math.max(0, upperBound(session.keyframes, safeTs) - 1);
  const frameTs = session.frameTimes[frameIndex] ?? 0;
  const keyframeTs = session.keyframes[keyframeIndex] ?? 0;
  const event = session.events[eventIndex] ?? session.events[0];
  const nearbyEvents = session.events.slice(
    Math.max(0, eventIndex - 4),
    Math.min(session.events.length, eventIndex + 5),
  );

  return {
    ts: safeTs,
    frameIndex,
    frameTs,
    keyframeIndex,
    keyframeTs,
    eventIndex,
    event,
    nearbyEvents,
  };
}

export function detectHighlights(
  events: ReplayEvent[],
  bookmarks: ReplayBookmark[] = [],
): ReplayHighlight[] {
  const highlights: ReplayHighlight[] = [];

  for (const event of events) {
    if (event.kind === "death") {
      highlights.push({
        id: `${event.id}-death`,
        category: "Deaths",
        ts: event.ts,
        title: `${event.actor} dies`,
        detail: event.detail,
        severity: "critical",
      });
    }

    if (event.kind === "wipe") {
      highlights.push({
        id: `${event.id}-wipe`,
        category: "Wipes",
        ts: event.ts,
        title: "Party wipe detected",
        detail: event.detail,
        severity: "critical",
      });
    }

    if (event.kind === "mez_break") {
      highlights.push({
        id: `${event.id}-mez-break`,
        category: "Mez breaks",
        ts: event.ts,
        title: "Mez break",
        detail: event.detail,
        severity: "warn",
      });
    }

    if (event.kind === "named_kill" || event.named) {
      highlights.push({
        id: `${event.id}-named-kill`,
        category: "Named kills",
        ts: event.ts,
        title: "Named enemy killed",
        detail: event.detail,
        severity: "info",
      });
    }

    if (event.kind === "full_clear") {
      highlights.push({
        id: `${event.id}-full-clear`,
        category: "Full-clears",
        ts: event.ts,
        title: "Camp full-clear",
        detail: event.detail,
        severity: "info",
      });
    }

    if (event.kind === "close_call" || (typeof event.hpPct === "number" && event.hpPct < 10)) {
      highlights.push({
        id: `${event.id}-close-call`,
        category: "Close calls",
        ts: event.ts,
        title: "Close call",
        detail: event.detail,
        severity: "warn",
      });
    }

    if (event.kind === "mez_chain") {
      highlights.push({
        id: `${event.id}-mez-chain`,
        category: "Long mez chains",
        ts: event.ts,
        endTs: event.ts + 36,
        title: "Long mez chain",
        detail: event.detail,
        severity: "info",
      });
    }
  }

  for (const bookmark of bookmarks) {
    highlights.push({
      id: bookmark.id,
      category: "Operator bookmarks",
      ts: bookmark.ts,
      title: bookmark.label,
      detail: bookmark.persistedLine,
      severity: "info",
    });
  }

  return highlights.sort((a, b) => a.ts - b.ts || a.title.localeCompare(b.title));
}

export function buildBookmark(ts: number, sessionId: string): ReplayBookmark {
  const line = JSON.stringify({
    sessionId,
    ts: Number(ts.toFixed(1)),
    kind: "bookmark",
    label: `Bookmark at ${formatClock(ts)}`,
    createdAt: new Date().toISOString(),
  });

  return {
    id: `bm-${Math.round(ts * 10)}-${sessionId}`,
    ts,
    label: `Bookmark at ${formatClock(ts)}`,
    createdAt: Date.now(),
    persistedLine: line,
  };
}
