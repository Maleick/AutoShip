import type {
  CampConfiguration,
  CampRecommendation,
  Estimate,
  Goal,
  ObjectiveWeights,
  Party,
  PerCharacter,
  RecommendRequest,
  Route,
  Group,
} from "../types";

export const DEFAULT_OBJECTIVE_WEIGHTS: ObjectiveWeights = {
  xp: 0.25,
  plat: 0.25,
  upgrades: 0.25,
  safety: 0.25,
};

export const OBJECTIVE_WEIGHT_PRESETS: Record<string, ObjectiveWeights> = {
  platRun: { xp: 0.15, plat: 0.5, upgrades: 0.2, safety: 0.15 },
  xpGrind: { xp: 0.55, plat: 0.1, upgrades: 0.2, safety: 0.15 },
  gearHunt: { xp: 0.15, plat: 0.15, upgrades: 0.55, safety: 0.15 },
  safeFarm: { xp: 0.2, plat: 0.15, upgrades: 0.15, safety: 0.5 },
};

export const DEFAULT_TOP_N = 5;
export const DEFAULT_EXPLORE_PCT = 0.15;
export const DEFAULT_MIN_CONFIDENCE = 0.5;

const FEAR_CLASS_ALLOWLIST = new Set([
  "bard",
  "beastlord",
  "cleric",
  "paladin",
  "shadow knight",
]);

export interface RecommendationOptions {
  topN?: number;
  explorePct?: number;
  seed?: string;
  rng?: RandomSource;
}

export type RandomSource = () => number;

export interface RecommendationTraceEntry {
  camp_id: string;
  stage: "filter" | "pareto" | "thompson";
  reason: string;
}

export interface PreparedCampRecommendation extends CampRecommendation {
  score: {
    xp: number;
    plat: number;
    upgrades: number;
    safety: number;
    travel: number;
    baseline: number;
    confidence: number;
    utility: number;
    thompson: number;
    sigma: number;
    normalized: {
      xp: number;
      plat: number;
      upgrades: number;
      safety: number;
      travel: number;
    };
  };
  trace: string[];
}

export interface RecommendationRun {
  prepared: PreparedCampRecommendation[];
  rejected: RecommendationTraceEntry[];
  topN: number;
  explorePct: number;
}

export interface RecommendationResult extends RecommendationRun {
  recommendations: CampRecommendation[];
}

export function normalizeObjectiveWeights(
  weights: ObjectiveWeights,
): ObjectiveWeights {
  const sum = weights.xp + weights.plat + weights.upgrades + weights.safety;
  if (!Number.isFinite(sum) || sum <= 0) {
    return DEFAULT_OBJECTIVE_WEIGHTS;
  }

  return {
    xp: weights.xp / sum,
    plat: weights.plat / sum,
    upgrades: weights.upgrades / sum,
    safety: weights.safety / sum,
  };
}

export function groupToParty(group: Group): Party {
  return {
    members: group.members.map((member) => ({
      name: member.character_name,
      class: member.class,
      role: member.role,
      level: member.level ?? 50,
    })),
  };
}

export function prepareRecommendations(
  request: RecommendRequest,
  camps: CampConfiguration[],
  options: RecommendationOptions = {},
): RecommendationRun {
  const topN = Math.max(1, options.topN ?? DEFAULT_TOP_N);
  const explorePct = clamp01(options.explorePct ?? DEFAULT_EXPLORE_PCT);
  const rng = options.rng ?? createSeededRng(options.seed ?? buildSeed(request, camps));
  const rejected: RecommendationTraceEntry[] = [];

  const withMetrics = camps
    .map((camp) => buildPreparedCampRecommendation(request, camp))
    .filter((candidate): candidate is PreparedCampRecommendation => {
      const rejection = hardConstraintRejection(request, candidate);
      if (rejection) {
        rejected.push({
          camp_id: candidate.camp_id,
          stage: "filter",
          reason: rejection,
        });
        return false;
      }
      return true;
    });

  const paretoFront = withMetrics.filter((candidate) => {
    const dominatedBy = withMetrics.find((other) =>
      other !== candidate && dominates(other, candidate),
    );
    if (dominatedBy) {
      rejected.push({
        camp_id: candidate.camp_id,
        stage: "pareto",
        reason: `dominated by ${dominatedBy.camp_id}`,
      });
      return false;
    }
    return true;
  });

  if (paretoFront.length === 0) {
    return {
      prepared: [],
      rejected,
      topN,
      explorePct,
    };
  }

  const poolSize = Math.min(topN, paretoFront.length);
  const exploreCount = explorePct <= 0 ? 0 : Math.max(1, Math.round(poolSize * explorePct));
  const selected = selectPreparedPool(paretoFront, poolSize, exploreCount, rng);

  return {
    prepared: selected,
    rejected,
    topN,
    explorePct,
  };
}

export function rerankRecommendations(
  prepared: PreparedCampRecommendation[],
  weights: ObjectiveWeights,
  topN = DEFAULT_TOP_N,
): CampRecommendation[] {
  const normalizedWeights = normalizeObjectiveWeights(weights);

  return [...prepared]
    .sort((left, right) => {
      const leftScore = weightedScore(left, normalizedWeights);
      const rightScore = weightedScore(right, normalizedWeights);
      if (rightScore !== leftScore) {
        return rightScore - leftScore;
      }
      if (right.score.thompson !== left.score.thompson) {
        return right.score.thompson - left.score.thompson;
      }
      if (right.score.confidence !== left.score.confidence) {
        return right.score.confidence - left.score.confidence;
      }
      return right.score.xp - left.score.xp;
    })
    .slice(0, topN)
    .map(stripPreparedRecommendation);
}

export function recommend(
  request: RecommendRequest,
  camps: CampConfiguration[],
  options: RecommendationOptions = {},
): RecommendationResult {
  const run = prepareRecommendations(request, camps, options);
  return {
    ...run,
    recommendations: rerankRecommendations(
      run.prepared,
      request.weights,
      run.topN,
    ),
  };
}

function buildPreparedCampRecommendation(
  request: RecommendRequest,
  camp: CampConfiguration,
): PreparedCampRecommendation {
  const route = estimateRoute(request.current_zone, camp);
  const risk = estimateRisk(request.party, camp, route.eta_min);
  const telemetry = estimateTelemetry(camp, route.eta_min, risk);
  const xpPerHr = estimateXpPerHr(request.goal, camp, route.eta_min, risk, telemetry.n);
  const ppPerHr = estimatePlatPerHr(request.goal, camp, route.eta_min, risk, telemetry.n);
  const upgradeProbability = estimateUpgradeProbability(request.party, request.goal, risk, telemetry.n);
  const averageUpgradeProbability = averageValues(upgradeProbability);
  const confidence = confidenceScore(telemetry.n, telemetry.sigma);
  const primaryAxis = primaryAxisForGoal(request.goal);
  const confidenceBand = buildConfidenceBand({
    axis: primaryAxis,
    xp: xpPerHr,
    pp: ppPerHr,
    upgradeProbability: averageUpgradeProbability,
  });
  const rationale = buildRationale({
    telemetrySamples: telemetry.n,
    upgradeProbability: averageUpgradeProbability,
    risk,
    etaMin: route.eta_min,
    goal: request.goal,
  });
  const normalizedTravel = 1 / (1 + route.eta_min);

  return {
    camp_id: camp.id,
    xp_per_hr: xpPerHr,
    pp_per_hr: ppPerHr,
    upgrade_probability: upgradeProbability,
    risk,
    eta_min: route.eta_min,
    route,
    rationale,
    confidence_band: confidenceBand,
    exploratory: false,
    score: {
      xp: xpPerHr.mu,
      plat: ppPerHr.mu,
      upgrades: averageUpgradeProbability,
      safety: 1 - risk,
      travel: normalizedTravel,
      baseline: 0,
      confidence,
      utility: 0,
      thompson: 0,
      sigma: telemetry.sigma,
      normalized: {
        xp: 0,
        plat: 0,
        upgrades: 0,
        safety: 0,
        travel: normalizedTravel,
      },
    },
    trace: [],
  };
}

function selectPreparedPool(
  candidates: PreparedCampRecommendation[],
  poolSize: number,
  exploreCount: number,
  rng: RandomSource,
): PreparedCampRecommendation[] {
  const working = candidates.map((candidate) => ({
    ...candidate,
    score: {
      ...candidate.score,
      normalized: { ...candidate.score.normalized },
    },
    trace: [...candidate.trace],
  }));

  const maxXp = Math.max(...working.map((candidate) => candidate.score.xp), 1);
  const maxPlat = Math.max(...working.map((candidate) => candidate.score.plat), 1);
  const maxUpgrade = Math.max(...working.map((candidate) => candidate.score.upgrades), 1e-6);

  for (const candidate of working) {
    candidate.score.normalized.xp = candidate.score.xp / maxXp;
    candidate.score.normalized.plat = candidate.score.plat / maxPlat;
    candidate.score.normalized.upgrades = candidate.score.upgrades / maxUpgrade;
    candidate.score.normalized.safety = candidate.score.safety;
    candidate.score.baseline = baselineUtility(candidate.score.normalized);
  }

  const sigmaValues = working.map((candidate) => candidate.score.sigma);
  const sigmaThreshold = median(sigmaValues);
  const explorationPool = working.filter(
    (candidate) => candidate.score.sigma >= sigmaThreshold || candidate.xp_per_hr.n <= median(working.map((item) => item.xp_per_hr.n)),
  );
  const explorationSource = explorationPool.length > 0 ? explorationPool : working;
  const exploratory = selectByThompson(explorationSource, exploreCount, rng);
  const exploratoryIds = new Set(exploratory.map((candidate) => candidate.camp_id));

  for (const candidate of working) {
    candidate.exploratory = exploratoryIds.has(candidate.camp_id);
    candidate.score.thompson = candidate.exploratory
      ? candidate.score.baseline + candidate.score.sigma * 0.35
      : candidate.score.baseline;
    candidate.score.utility = candidate.score.baseline;
  }

  const selected = [...exploratory];
  const remainingSlots = Math.max(0, poolSize - selected.length);
  const exploitationPool = working
    .filter((candidate) => !exploratoryIds.has(candidate.camp_id))
    .sort((left, right) => {
      if (right.score.baseline !== left.score.baseline) {
        return right.score.baseline - left.score.baseline;
      }
      return right.score.confidence - left.score.confidence;
    })
    .slice(0, remainingSlots);

  selected.push(...exploitationPool);

  return selected.slice(0, poolSize);
}

function selectByThompson(
  candidates: PreparedCampRecommendation[],
  count: number,
  rng: RandomSource,
): PreparedCampRecommendation[] {
  if (count <= 0) {
    return [];
  }

  const ranked = candidates
    .map((candidate) => {
      const sigma = Math.max(candidate.score.sigma, 0.05);
      const sample = sampleNormal(candidate.score.baseline, sigma, rng);
      return {
        candidate,
        sample,
      };
    })
    .sort((left, right) => {
      if (right.sample !== left.sample) {
        return right.sample - left.sample;
      }
      if (right.candidate.score.sigma !== left.candidate.score.sigma) {
        return right.candidate.score.sigma - left.candidate.score.sigma;
      }
      return right.candidate.score.confidence - left.candidate.score.confidence;
    });

  return ranked.slice(0, count).map((entry) => ({
    ...entry.candidate,
    score: {
      ...entry.candidate.score,
      thompson: entry.sample,
    },
  }));
}

function hardConstraintRejection(
  request: RecommendRequest,
  candidate: PreparedCampRecommendation,
): string | null {
  if (
    request.time_budget_min !== undefined &&
    candidate.eta_min > request.time_budget_min
  ) {
    return `short travel: eta ${candidate.eta_min}m exceeds budget ${request.time_budget_min}m`;
  }

  if (candidate.score.confidence < request.min_confidence) {
    return `confidence ${candidate.score.confidence.toFixed(2)} below minimum ${request.min_confidence.toFixed(2)}`;
  }

  if (requiresFearClass(candidate) && !hasFearClass(request.party)) {
    return "fear zone requires a fear-capable class";
  }

  const levelBand = estimateLevelBand(candidate);
  const partyAverage = averagePartyLevel(request.party);
  if (partyAverage !== null && partyAverage < levelBand.min) {
    return `level mismatch: party average ${partyAverage.toFixed(1)} below band ${levelBand.min}-${levelBand.max}`;
  }
  if (partyAverage !== null && partyAverage > levelBand.max) {
    return `level mismatch: party average ${partyAverage.toFixed(1)} above band ${levelBand.min}-${levelBand.max}`;
  }

  return null;
}

function dominates(
  left: PreparedCampRecommendation,
  right: PreparedCampRecommendation,
): boolean {
  const leftUpgrade = averageValues(left.upgrade_probability);
  const rightUpgrade = averageValues(right.upgrade_probability);
  const leftSafety = 1 - left.risk;
  const rightSafety = 1 - right.risk;
  const leftTravel = 1 / (1 + left.eta_min);
  const rightTravel = 1 / (1 + right.eta_min);

  const betterOrEqual =
    left.xp_per_hr.mu >= right.xp_per_hr.mu &&
    left.pp_per_hr.mu >= right.pp_per_hr.mu &&
    leftUpgrade >= rightUpgrade &&
    leftSafety >= rightSafety &&
    leftTravel >= rightTravel;

  const strictlyBetter =
    left.xp_per_hr.mu > right.xp_per_hr.mu ||
    left.pp_per_hr.mu > right.pp_per_hr.mu ||
    leftUpgrade > rightUpgrade ||
    leftSafety > rightSafety ||
    leftTravel > rightTravel;

  return betterOrEqual && strictlyBetter;
}

function weightedScore(
  candidate: PreparedCampRecommendation,
  weights: ObjectiveWeights,
): number {
  return (
    candidate.score.normalized.xp * weights.xp +
    candidate.score.normalized.plat * weights.plat +
    candidate.score.normalized.upgrades * weights.upgrades +
    candidate.score.normalized.safety * weights.safety
  );
}

function stripPreparedRecommendation(
  candidate: PreparedCampRecommendation,
): CampRecommendation {
  return {
    camp_id: candidate.camp_id,
    xp_per_hr: candidate.xp_per_hr,
    pp_per_hr: candidate.pp_per_hr,
    upgrade_probability: candidate.upgrade_probability,
    risk: candidate.risk,
    eta_min: candidate.eta_min,
    route: candidate.route,
    rationale: candidate.rationale,
    confidence_band: candidate.confidence_band,
    exploratory: candidate.exploratory,
  };
}

function estimateRoute(currentZone: string, camp: CampConfiguration): Route {
  const current = normalizeZone(currentZone);
  const target = normalizeZone(camp.camp_zone);
  const sharedPrefix = longestSharedPrefix(current, target);
  const sameZone = current === target && current.length > 0;
  const prefixFactor = sharedPrefix >= 5 ? 0.8 : sharedPrefix >= 3 ? 1.4 : 2.4;
  const radiusFactor = Math.max(1, camp.pull_radius / 120);
  const safetyFactor = camp.safe_zone_markers.length * 0.4;
  const etaMin = Math.max(
    3,
    Math.round((sameZone ? 4 : 8) + prefixFactor * 2 + radiusFactor + safetyFactor),
  );

  return {
    from_zone: currentZone,
    to_zone: camp.camp_zone,
    eta_min: etaMin,
    path: sameZone
      ? [currentZone, camp.camp_zone]
      : [currentZone || "unknown", "camp route", camp.camp_zone],
  };
}

function estimateRisk(
  party: Party,
  camp: CampConfiguration,
  etaMin: number,
): number {
  const partyRoles = new Set(party.members.map((member) => member.role));
  const roleCoverage =
    (partyRoles.has("main_tank") ? 0.04 : 0) +
    (partyRoles.has("healer") ? 0.04 : 0) +
    (partyRoles.has("puller") ? 0.03 : 0);
  const strategyPenalty =
    camp.combat_settings.pull_strategy === "caster"
      ? 0.05
      : camp.combat_settings.pull_strategy === "melee"
        ? 0.02
        : 0;
  const safetyBonus = camp.safe_zone_markers.length * 0.03;
  const targetBonus = camp.pull_targets.filter((target) => target.enabled).length * 0.01;
  const fearPenalty = requiresFearClass(camp) ? 0.1 : 0;
  const distancePenalty = Math.min(0.25, etaMin / 120);

  return clamp01(
    0.14 +
      camp.pull_radius / 1200 +
      strategyPenalty +
      targetBonus -
      roleCoverage -
      safetyBonus +
      fearPenalty +
      distancePenalty,
  );
}

function estimateTelemetry(
  camp: CampConfiguration,
  etaMin: number,
  risk: number,
): Estimate {
  const n =
    1 +
    camp.pull_points.filter((point) => point.enabled).length +
    camp.pull_targets.filter((target) => target.enabled).length +
    camp.safe_zone_markers.length;
  const sigma = clamp(
    4,
    18,
    16 - n * 0.9 + risk * 6 + etaMin * 0.12,
  );

  return {
    mu: n,
    sigma,
    n,
  };
}

function estimateXpPerHr(
  goal: Goal,
  camp: CampConfiguration,
  etaMin: number,
  risk: number,
  telemetrySamples: number,
): Estimate {
  const goalBonus = goal.kind === "xp" ? 18 : goal.kind === "plat" ? 4 : 8;
  const targetDensity = camp.pull_targets.filter((target) => target.enabled).length * 11;
  const pullShape = camp.pull_points.filter((point) => point.enabled).length * 4;
  const mu = Math.max(
    10,
    60 + targetDensity + pullShape - etaMin * 1.8 - risk * 22 + goalBonus,
  );
  const sigma = clamp(4, 22, 12 - telemetrySamples * 0.45 + risk * 5 + etaMin * 0.1);

  return { mu, sigma, n: telemetrySamples };
}

function estimatePlatPerHr(
  goal: Goal,
  camp: CampConfiguration,
  etaMin: number,
  risk: number,
  telemetrySamples: number,
): Estimate {
  const goalBonus = goal.kind === "plat" ? 18 : goal.kind === "item" ? 8 : 0;
  const lootDensity =
    camp.pull_targets.filter((target) => target.enabled).length * 9 +
    camp.safe_zone_markers.length * 3;
  const mu = Math.max(
    8,
    45 + lootDensity + camp.pull_radius * 0.08 - etaMin * 1.2 - risk * 18 + goalBonus,
  );
  const sigma = clamp(4, 20, 10 - telemetrySamples * 0.35 + risk * 4 + etaMin * 0.08);

  return { mu, sigma, n: telemetrySamples };
}

function estimateUpgradeProbability(
  party: Party,
  goal: Goal,
  risk: number,
  telemetrySamples: number,
): PerCharacter<number> {
  const targetCharacter =
    goal.kind === "item" ? goal.character.toLowerCase() : null;

  return party.members.reduce<PerCharacter<number>>((accumulator, member) => {
    const roleBoost =
      member.role === "main_tank"
        ? 0.09
        : member.role === "healer"
          ? 0.11
          : member.role === "puller"
            ? 0.08
            : member.role === "cc"
              ? 0.07
              : 0.06;
    const goalBoost =
      goal.kind === "item" && member.name.toLowerCase() === targetCharacter
        ? 0.32
        : goal.kind === "faction"
          ? 0.16
          : goal.kind === "plat"
            ? 0.08
            : 0.05;
    const telemetryBoost = Math.min(0.12, telemetrySamples * 0.01);
    const value = clamp01(0.08 + roleBoost + goalBoost + telemetryBoost - risk * 0.15);
    accumulator[member.name] = value;
    return accumulator;
  }, {});
}

function buildRationale(params: {
  telemetrySamples: number;
  upgradeProbability: number;
  risk: number;
  etaMin: number;
  goal: Goal;
}): string {
  const reasons: string[] = [];
  if (params.telemetrySamples >= 8) {
    reasons.push("telemetry advantage");
  }
  if (params.upgradeProbability >= 0.35) {
    reasons.push("upgrade slot");
  }
  if (params.risk <= 0.35) {
    reasons.push("low risk");
  }
  if (params.etaMin <= 10) {
    reasons.push("short travel");
  }

  if (reasons.length === 0) {
    reasons.push(
      params.goal.kind === "xp"
        ? "balanced XP yield"
        : params.goal.kind === "plat"
          ? "balanced plat yield"
          : "balanced camp fit",
    );
  }

  return reasons.slice(0, 2).join(", ");
}

function buildConfidenceBand(params: {
  axis: "xp" | "plat" | "upgrades";
  xp: Estimate;
  pp: Estimate;
  upgradeProbability: number;
}): [number, number] {
  if (params.axis === "upgrades") {
    const sigma = Math.sqrt(
      Math.max(params.upgradeProbability * (1 - params.upgradeProbability), 0) /
        10,
    );
    return [
      clamp01(params.upgradeProbability - 1.96 * sigma),
      clamp01(params.upgradeProbability + 1.96 * sigma),
    ];
  }

  const estimate = params.axis === "xp" ? params.xp : params.pp;
  const radius = 1.96 * estimate.sigma;
  return [Math.max(0, estimate.mu - radius), estimate.mu + radius];
}

function primaryAxisForGoal(goal: Goal): "xp" | "plat" | "upgrades" {
  if (goal.kind === "xp") return "xp";
  if (goal.kind === "plat") return "plat";
  return "upgrades";
}

function averageValues(values: PerCharacter<number>): number {
  const entries = Object.values(values);
  if (entries.length === 0) {
    return 0;
  }
  return entries.reduce((sum, value) => sum + value, 0) / entries.length;
}

function confidenceScore(samples: number, sigma: number): number {
  return clamp01(samples / (samples + sigma / 8));
}

function averagePartyLevel(party: Party): number | null {
  const levels = party.members
    .map((member) => member.level)
    .filter((level): level is number => typeof level === "number" && Number.isFinite(level));

  if (levels.length === 0) {
    return null;
  }

  return levels.reduce((sum, level) => sum + level, 0) / levels.length;
}

function estimateLevelBand(candidate: PreparedCampRecommendation): { min: number; max: number } {
  const base = Math.max(
    1,
    Math.round(38 + candidate.score.safety * 14 + candidate.score.travel * 10),
  );
  return {
    min: base,
    max: base + 16,
  };
}

function requiresFearClass(input: CampConfiguration | PreparedCampRecommendation): boolean {
  if ("requires_fear_class" in input && input.requires_fear_class) {
    return true;
  }
  const zone = "camp_zone" in input ? input.camp_zone : input.route.to_zone;
  return zone.toLowerCase().includes("fear");
}

function hasFearClass(party: Party): boolean {
  return party.members.some((member) =>
    FEAR_CLASS_ALLOWLIST.has(member.class.toLowerCase()),
  );
}

function buildSeed(request: RecommendRequest, camps: CampConfiguration[]): string {
  const partyPart = request.party.members
    .map((member) => `${member.name}:${member.class}:${member.role}`)
    .join("|");
  const campPart = camps.map((camp) => camp.id).join("|");
  return [
    request.current_zone,
    request.goal.kind,
    JSON.stringify(request.goal),
    partyPart,
    campPart,
  ].join("::");
}

function createSeededRng(seed: string): RandomSource {
  let state = hashString(seed) || 0x9e3779b9;
  return () => {
    state = (state + 0x6d2b79f5) | 0;
    let value = Math.imul(state ^ (state >>> 15), 1 | state);
    value ^= value + Math.imul(value ^ (value >>> 7), 61 | value);
    return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
  };
}

function hashString(input: string): number {
  let hash = 0x811c9dc5;
  for (let index = 0; index < input.length; index += 1) {
    hash ^= input.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

function sampleNormal(mean: number, sigma: number, rng: RandomSource): number {
  const u1 = Math.max(rng(), Number.EPSILON);
  const u2 = Math.max(rng(), Number.EPSILON);
  const mag = Math.sqrt(-2.0 * Math.log(u1));
  const z0 = mag * Math.cos(2.0 * Math.PI * u2);
  return mean + z0 * sigma;
}

function normalizeZone(zone: string): string {
  return zone.trim().toLowerCase().replace(/[^a-z0-9]+/g, "");
}

function longestSharedPrefix(left: string, right: string): number {
  let index = 0;
  while (index < left.length && index < right.length && left[index] === right[index]) {
    index += 1;
  }
  return index;
}

function median(values: number[]): number {
  if (values.length === 0) {
    return 0;
  }
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? (sorted[middle - 1] + sorted[middle]) / 2
    : sorted[middle];
}

function baselineUtility(normalized: {
  xp: number;
  plat: number;
  upgrades: number;
  safety: number;
  travel: number;
}): number {
  return (
    normalized.xp * 0.25 +
    normalized.plat * 0.25 +
    normalized.upgrades * 0.25 +
    normalized.safety * 0.15 +
    normalized.travel * 0.1
  );
}

function clamp(min: number, max: number, value: number): number {
  return Math.max(min, Math.min(max, value));
}

function clamp01(value: number): number {
  return clamp(0, 1, value);
}
