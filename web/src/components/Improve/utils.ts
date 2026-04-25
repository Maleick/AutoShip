import {
  Session,
  SessionCampFingerprint,
  PersonalBest,
  ComparisonPoint,
} from "./types";

// Camp fingerprint: deterministic hash of zone + mob set + party comp + level range
export const computeCampFingerprint = (
  session: Session,
): SessionCampFingerprint => {
  const key = [
    session.camp.zone,
    session.camp.mobSet.sort().join(","),
    session.camp.partyComp.sort().join(","),
    session.camp.levelRange,
  ].join("|");

  // Simple hash function
  let hash = 0;
  for (let i = 0; i < key.length; i++) {
    const char = key.charCodeAt(i);
    hash = (hash << 5) - hash + char;
    hash = hash & hash; // Convert to 32bit integer
  }

  return {
    campId: Math.abs(hash).toString(16),
    zone: session.camp.zone,
    mobSet: session.camp.mobSet,
    partyComp: session.camp.partyComp,
    levelRange: session.camp.levelRange,
  };
};

// Time decay: mark baseline stale after 60 days or patch event
export const isBaselineStale = (baseline: PersonalBest): boolean => {
  if (baseline.staleAt) {
    return new Date() > new Date(baseline.staleAt);
  }

  const sixtyDaysAgo = new Date();
  sixtyDaysAgo.setDate(sixtyDaysAgo.getDate() - 60);
  return new Date(baseline.recordedAt) < sixtyDaysAgo;
};

// Find personal best for a camp; prefer pinned, then most recent non-stale
export const findPersonalBest = (
  baselines: PersonalBest[],
  campId: string,
  metric: "xphour" | "dps" | "deaths",
): PersonalBest | null => {
  const candidates = baselines.filter(
    (b) => b.campId === campId && b.metric === metric && !isBaselineStale(b),
  );

  if (candidates.length === 0) return null;

  const pinned = candidates.find((b) => b.isPinned);
  if (pinned) return pinned;

  return candidates.sort(
    (a, b) =>
      new Date(b.recordedAt).getTime() - new Date(a.recordedAt).getTime(),
  )[0];
};

// Compute delta relative to personal best; suppress if within noise floor
export const computeDelta = (
  current: number,
  personalBest: number | null,
  noiseFloor: number = 0.01, // 1% default
): ComparisonPoint | null => {
  if (personalBest === null) return null;

  const delta = ((current - personalBest) / personalBest) * 100;
  const isWithinNoise = Math.abs(delta) < noiseFloor * 100;

  return {
    label: "vs personal best",
    current,
    personal_best: personalBest,
    delta,
    isWithinNoise,
  };
};

// Format duration (ms) to human-readable
export const formatDuration = (ms: number): string => {
  const sec = Math.floor(ms / 1000);
  const min = Math.floor(sec / 60);
  const hr = Math.floor(min / 60);

  if (hr > 0) return `${hr}h ${min % 60}m`;
  if (min > 0) return `${min}m ${sec % 60}s`;
  return `${sec}s`;
};

// Format metric with appropriate precision
export const formatMetric = (
  value: number,
  type: "dps" | "healing" | "xphour" | "percent",
): string => {
  if (type === "percent") return `${value.toFixed(1)}%`;
  if (type === "dps" || type === "healing") return `${Math.round(value)}`;
  if (type === "xphour") return `${Math.round(value)}/hr`;
  return value.toString();
};
