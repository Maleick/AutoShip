import { useEffect, useState } from "react";
import {
  Clock,
  MapPin,
  Shield,
  Sparkle,
  Target,
} from "@phosphor-icons/react";

import { Badge, Card } from "./ui";
import type {
  CampConfiguration,
  CampRecommendation,
  Goal,
  Group,
  ObjectiveWeights,
  RecommendRequest,
} from "../types";
import {
  DEFAULT_EXPLORE_PCT,
  DEFAULT_MIN_CONFIDENCE,
  DEFAULT_OBJECTIVE_WEIGHTS,
  DEFAULT_TOP_N,
  OBJECTIVE_WEIGHT_PRESETS,
  groupToParty,
  normalizeObjectiveWeights,
  prepareRecommendations,
  rerankRecommendations,
  type RecommendationRun,
} from "../lib/camp-recommender";

interface CampRecommendationsPanelProps {
  selectedGroup: Group | null;
  campConfigs: CampConfiguration[];
}

type GoalMode = Goal["kind"];

function goalLabel(goal: GoalMode) {
  switch (goal) {
    case "xp":
      return "XP Grind";
    case "plat":
      return "Plat Run";
    case "item":
      return "Gear Hunt";
    case "faction":
      return "Faction Push";
  }
}

function buildGoal(
  goal: GoalMode,
  slot: string,
  character: string,
  faction: string,
): Goal {
  switch (goal) {
    case "item":
      return {
        kind: "item",
        slot: slot.trim() || "slot",
        character: character.trim() || "target",
      };
    case "faction":
      return {
        kind: "faction",
        faction: faction.trim() || "default",
      };
    default:
      return { kind: goal };
  }
}

function formatPercent(value: number) {
  return `${Math.round(value * 100)}%`;
}

function formatWeight(value: number) {
  return `${Math.round(value * 100)}%`;
}

function memberLevelHint(group: Group | null) {
  if (!group) {
    return "Select a group to score camps.";
  }

  if (group.members.length === 0) {
    return "This group has no members yet.";
  }

  const withLevels = group.members.filter((member) => member.level !== undefined);
  if (withLevels.length === 0) {
    return "Party levels default to 50 until explicit levels are set.";
  }

  const average =
    withLevels.reduce((sum, member) => sum + (member.level ?? 0), 0) /
    withLevels.length;
  return `Average party level: ${average.toFixed(1)}`;
}

function CampRecommendationCard({
  recommendation,
  camp,
}: {
  recommendation: CampRecommendation;
  camp: CampConfiguration | undefined;
}) {
  const topUpgrades = Object.entries(recommendation.upgrade_probability)
    .sort((left, right) => right[1] - left[1])
    .slice(0, 3);

  return (
    <Card
      variant="elevated"
      className={`rounded-[1.25rem] border ${
        recommendation.exploratory
          ? "border-cyan-300/30 bg-cyan-500/10"
          : "border-white/10 bg-white/5"
      }`}
      title={camp?.template_name ?? camp?.camp_zone ?? recommendation.camp_id}
      subtitle={`${recommendation.route.to_zone} - ETA ${recommendation.eta_min}m`}
      actions={
        <div className="flex items-center gap-2">
          {recommendation.exploratory ? (
            <Badge variant="status-idle">Uncharted</Badge>
          ) : (
            <Badge variant="status-active">Stable</Badge>
          )}
        </div>
      }
    >
      <div className="grid gap-3 text-sm text-white/80">
        <p className="text-white/70">{recommendation.rationale}</p>

        <div className="grid grid-cols-2 gap-3">
          <div className="rounded-xl border border-white/10 bg-black/20 p-3">
            <div className="flex items-center gap-2 text-cyan-200">
              <Target size={14} />
              <span className="text-[10px] uppercase tracking-[0.28em]">
                XP / hr
              </span>
            </div>
            <div className="mt-2 text-lg font-semibold text-cyan-100">
              {recommendation.xp_per_hr.mu.toFixed(1)}
            </div>
            <div className="mt-1 text-xs text-cyan-100/60">
              CI {recommendation.confidence_band[0].toFixed(1)} -{" "}
              {recommendation.confidence_band[1].toFixed(1)}
            </div>
          </div>

          <div className="rounded-xl border border-white/10 bg-black/20 p-3">
            <div className="flex items-center gap-2 text-amber-200">
              <Shield size={14} />
              <span className="text-[10px] uppercase tracking-[0.28em]">
                Safety
              </span>
            </div>
            <div className="mt-2 text-lg font-semibold text-amber-100">
              {(1 - recommendation.risk).toFixed(2)}
            </div>
            <div className="mt-1 text-xs text-amber-100/60">
              Risk {formatPercent(recommendation.risk)}
            </div>
          </div>
        </div>

        <div className="grid grid-cols-2 gap-3">
          <div className="rounded-xl border border-white/10 bg-black/20 p-3">
            <div className="flex items-center gap-2 text-fuchsia-200">
              <Sparkle size={14} />
              <span className="text-[10px] uppercase tracking-[0.28em]">
                Upgrades
              </span>
            </div>
            <div className="mt-2 space-y-1">
              {topUpgrades.length > 0 ? (
                topUpgrades.map(([name, value]) => (
                  <div
                    key={name}
                    className="flex items-center justify-between text-xs text-white/70"
                  >
                    <span>{name}</span>
                    <span>{formatPercent(value)}</span>
                  </div>
                ))
              ) : (
                <p className="text-xs text-white/45">No upgrade targets.</p>
              )}
            </div>
          </div>

          <div className="rounded-xl border border-white/10 bg-black/20 p-3">
            <div className="flex items-center gap-2 text-emerald-200">
              <MapPin size={14} />
              <span className="text-[10px] uppercase tracking-[0.28em]">
                Travel
              </span>
            </div>
            <div className="mt-2 text-lg font-semibold text-emerald-100">
              {recommendation.route.eta_min}m
            </div>
            <div className="mt-1 text-xs text-emerald-100/60">
              {recommendation.route.path.join(" -> ")}
            </div>
          </div>
        </div>

        <div className="rounded-xl border border-white/10 bg-black/20 p-3">
          <div className="flex items-center gap-2 text-white/65">
            <Clock size={14} />
            <span className="text-[10px] uppercase tracking-[0.28em]">
              PP / hr
            </span>
          </div>
          <div className="mt-2 text-base font-semibold text-white">
            {recommendation.pp_per_hr.mu.toFixed(1)}
          </div>
        </div>
      </div>
    </Card>
  );
}

export function CampRecommendationsPanel({
  selectedGroup,
  campConfigs,
}: CampRecommendationsPanelProps) {
  const [goal, setGoal] = useState<GoalMode>("xp");
  const [goalSlot, setGoalSlot] = useState("chest");
  const [goalCharacter, setGoalCharacter] = useState("");
  const [goalFaction, setGoalFaction] = useState("");
  const [currentZone, setCurrentZone] = useState(selectedGroup?.zone ?? "");
  const [timeBudgetMin, setTimeBudgetMin] = useState(30);
  const [minConfidence, setMinConfidence] = useState(DEFAULT_MIN_CONFIDENCE);
  const [explorePct, setExplorePct] = useState(DEFAULT_EXPLORE_PCT);
  const [topN, setTopN] = useState(DEFAULT_TOP_N);
  const [weights, setWeights] = useState<ObjectiveWeights>(
    DEFAULT_OBJECTIVE_WEIGHTS,
  );
  const [preparedRun, setPreparedRun] = useState<RecommendationRun | null>(
    null,
  );

  useEffect(() => {
    setCurrentZone(selectedGroup?.zone ?? campConfigs[0]?.camp_zone ?? "");
  }, [campConfigs, selectedGroup]);

  useEffect(() => {
    if (!selectedGroup || campConfigs.length === 0) {
      setPreparedRun(null);
      return;
    }

    const request: RecommendRequest = {
      party: groupToParty(selectedGroup),
      current_zone:
        currentZone.trim() || selectedGroup.zone || campConfigs[0]?.camp_zone || "unknown",
      goal: buildGoal(goal, goalSlot, goalCharacter, goalFaction),
      time_budget_min: timeBudgetMin > 0 ? timeBudgetMin : undefined,
      weights: DEFAULT_OBJECTIVE_WEIGHTS,
      min_confidence: minConfidence,
    };

    const seed = [
      selectedGroup.id,
      currentZone,
      goal,
      goalSlot,
      goalCharacter,
      goalFaction,
      timeBudgetMin,
      minConfidence,
      campConfigs.map((camp) => camp.id).join("|"),
    ].join("::");

    setPreparedRun(
      prepareRecommendations(request, campConfigs, {
        topN,
        explorePct,
        seed,
      }),
    );
  }, [
    campConfigs,
    currentZone,
    explorePct,
    goal,
    goalCharacter,
    goalFaction,
    goalSlot,
    minConfidence,
    selectedGroup,
    timeBudgetMin,
    topN,
  ]);

  const normalizedWeights = normalizeObjectiveWeights(weights);
  const recommendations = preparedRun
    ? rerankRecommendations(
        preparedRun.prepared,
        normalizedWeights,
        topN,
      )
    : [];

  const campLookup = new Map(campConfigs.map((camp) => [camp.id, camp]));

  if (!selectedGroup) {
    return (
      <Card
        variant="elevated"
        className="rounded-[1.5rem] border border-white/10 bg-[#120a1d]/72"
        title="Camp Recommendations"
        subtitle="Select a group to rank camps"
      >
        <div className="text-sm text-white/50">
          {memberLevelHint(selectedGroup)}
        </div>
      </Card>
    );
  }

  return (
    <Card
      variant="elevated"
      className="rounded-[1.75rem] border border-white/10 bg-[#120a1d]/72"
      title="Camp Recommendations"
      subtitle={`${selectedGroup.name} - ${goalLabel(goal)} - top ${topN}`}
      actions={<Badge variant="status-active">Live ranker</Badge>}
    >
      <div className="grid gap-5">
        <div className="grid gap-3 xl:grid-cols-[1.2fr_0.8fr]">
          <div className="rounded-2xl border border-white/10 bg-black/20 p-4">
            <div className="mb-3 flex items-center justify-between">
              <div>
                <h3 className="font-archaic text-lg text-white">
                  Operator weights
                </h3>
                <p className="text-xs uppercase tracking-[0.24em] text-white/45 font-tech">
                  Simplex sliders rerank the prepared pool only
                </p>
              </div>
              <div className="flex flex-wrap gap-2">
                {Object.entries(OBJECTIVE_WEIGHT_PRESETS).map(([name, preset]) => (
                  <button
                    key={name}
                    type="button"
                    onClick={() => setWeights(preset)}
                    className="rounded-full border border-white/15 px-3 py-1 text-[10px] uppercase tracking-[0.24em] text-white/60 transition-colors hover:border-cyan-300/30 hover:text-cyan-100"
                  >
                    {name === "platRun"
                      ? "Plat Run"
                      : name === "xpGrind"
                        ? "XP Grind"
                        : name === "gearHunt"
                          ? "Gear Hunt"
                          : "Safe Farm"}
                  </button>
                ))}
              </div>
            </div>

            <div className="grid gap-3 sm:grid-cols-2">
              <SliderField
                label="XP"
                value={weights.xp}
                onChange={(value) => setWeights({ ...weights, xp: value })}
              />
              <SliderField
                label="PP"
                value={weights.plat}
                onChange={(value) => setWeights({ ...weights, plat: value })}
              />
              <SliderField
                label="Upgrades"
                value={weights.upgrades}
                onChange={(value) => setWeights({ ...weights, upgrades: value })}
              />
              <SliderField
                label="Safety"
                value={weights.safety}
                onChange={(value) => setWeights({ ...weights, safety: value })}
              />
            </div>

            <div className="mt-3 text-xs uppercase tracking-[0.24em] text-white/45">
              Normalized mix: XP {formatWeight(normalizedWeights.xp)} / PP{" "}
              {formatWeight(normalizedWeights.plat)} / Upgrades{" "}
              {formatWeight(normalizedWeights.upgrades)} / Safety{" "}
              {formatWeight(normalizedWeights.safety)}
            </div>
          </div>

          <div className="rounded-2xl border border-white/10 bg-black/20 p-4">
            <div className="mb-3">
              <h3 className="font-archaic text-lg text-white">Request</h3>
              <p className="text-xs uppercase tracking-[0.24em] text-white/45 font-tech">
                {memberLevelHint(selectedGroup)}
              </p>
            </div>

            <div className="grid gap-3 text-sm text-white/70">
              <div className="grid gap-2">
                <span className="text-[10px] uppercase tracking-[0.24em] text-white/45">
                  Goal
                </span>
                <div className="flex flex-wrap gap-2">
                  {(["xp", "plat", "item", "faction"] as GoalMode[]).map(
                    (value) => (
                      <button
                        key={value}
                        type="button"
                        onClick={() => setGoal(value)}
                        className={`rounded-full border px-3 py-1 text-[10px] uppercase tracking-[0.24em] transition-colors ${
                          goal === value
                            ? "border-cyan-300/40 bg-cyan-400/10 text-cyan-100"
                            : "border-white/10 bg-black/20 text-white/55 hover:border-white/25 hover:text-white/80"
                        }`}
                      >
                        {goalLabel(value)}
                      </button>
                    ),
                  )}
                </div>
              </div>

              {goal === "item" && (
                <div className="grid grid-cols-2 gap-3">
                  <label className="grid gap-2">
                    <span className="text-[10px] uppercase tracking-[0.24em] text-white/45">
                      Slot
                    </span>
                    <input
                      value={goalSlot}
                      onChange={(event) => setGoalSlot(event.target.value)}
                      className="rounded-xl border border-white/10 bg-black/30 px-3 py-2 text-white outline-none focus:border-cyan-300/40"
                      placeholder="Chest"
                    />
                  </label>
                  <label className="grid gap-2">
                    <span className="text-[10px] uppercase tracking-[0.24em] text-white/45">
                      Character
                    </span>
                    <input
                      value={goalCharacter}
                      onChange={(event) => setGoalCharacter(event.target.value)}
                      className="rounded-xl border border-white/10 bg-black/30 px-3 py-2 text-white outline-none focus:border-cyan-300/40"
                      placeholder="Frostreaver"
                    />
                  </label>
                </div>
              )}

              {goal === "faction" && (
                <label className="grid gap-2">
                  <span className="text-[10px] uppercase tracking-[0.24em] text-white/45">
                    Faction
                  </span>
                  <input
                    value={goalFaction}
                    onChange={(event) => setGoalFaction(event.target.value)}
                    className="rounded-xl border border-white/10 bg-black/30 px-3 py-2 text-white outline-none focus:border-cyan-300/40"
                    placeholder="Crushbone Orcs"
                  />
                </label>
              )}

              <label className="grid gap-2">
                <span className="text-[10px] uppercase tracking-[0.24em] text-white/45">
                  Current zone
                </span>
                <input
                  value={currentZone}
                  onChange={(event) => setCurrentZone(event.target.value)}
                  className="rounded-xl border border-white/10 bg-black/30 px-3 py-2 text-white outline-none focus:border-cyan-300/40"
                  placeholder="Enter current zone"
                />
              </label>

              <div className="grid grid-cols-2 gap-3">
                <label className="grid gap-2">
                  <span className="text-[10px] uppercase tracking-[0.24em] text-white/45">
                    Top-N
                  </span>
                  <input
                    type="number"
                    min={1}
                    max={10}
                    value={topN}
                    onChange={(event) =>
                      setTopN(Math.max(1, Number.parseInt(event.target.value || "1", 10)))
                    }
                    className="rounded-xl border border-white/10 bg-black/30 px-3 py-2 text-white outline-none focus:border-cyan-300/40"
                  />
                </label>

                <label className="grid gap-2">
                  <span className="text-[10px] uppercase tracking-[0.24em] text-white/45">
                    Time budget
                  </span>
                  <input
                    type="number"
                    min={1}
                    value={timeBudgetMin}
                    onChange={(event) =>
                      setTimeBudgetMin(
                        Math.max(1, Number.parseInt(event.target.value || "1", 10)),
                      )
                    }
                    className="rounded-xl border border-white/10 bg-black/30 px-3 py-2 text-white outline-none focus:border-cyan-300/40"
                  />
                </label>
              </div>

              <label className="grid gap-2">
                <span className="text-[10px] uppercase tracking-[0.24em] text-white/45">
                  Min confidence {formatPercent(minConfidence)}
                </span>
                <input
                  type="range"
                  min={0}
                  max={1}
                  step={0.05}
                  value={minConfidence}
                  onChange={(event) => setMinConfidence(Number(event.target.value))}
                />
              </label>

              <label className="grid gap-2">
                <span className="text-[10px] uppercase tracking-[0.24em] text-white/45">
                  Explore budget {formatPercent(explorePct)}
                </span>
                <input
                  type="range"
                  min={0}
                  max={0.5}
                  step={0.01}
                  value={explorePct}
                  onChange={(event) => setExplorePct(Number(event.target.value))}
                />
              </label>
            </div>
          </div>
        </div>

        <div className="grid gap-4">
          {recommendations.length === 0 ? (
            <div className="rounded-2xl border border-white/10 bg-black/20 p-5 text-sm text-white/55">
              No recommendation pool survived the current constraints.
            </div>
          ) : (
            recommendations.map((recommendation) => (
              <CampRecommendationCard
                key={recommendation.camp_id}
                recommendation={recommendation}
                camp={campLookup.get(recommendation.camp_id)}
              />
            ))
          )}
        </div>

        <details className="rounded-2xl border border-white/10 bg-black/20 p-4 text-sm text-white/65">
          <summary className="cursor-pointer text-[10px] uppercase tracking-[0.28em] text-white/45">
            Trace
          </summary>
          <div className="mt-3 space-y-2">
            {preparedRun?.rejected.length ? (
              preparedRun.rejected.map((entry) => (
                <div key={`${entry.camp_id}-${entry.stage}-${entry.reason}`}>
                  <span className="text-white/45">{entry.stage}</span>{" "}
                  <span className="text-white/80">{entry.camp_id}</span>{" "}
                  <span className="text-white/50">- {entry.reason}</span>
                </div>
              ))
            ) : (
              <p className="text-white/45">No rejections were recorded.</p>
            )}
          </div>
        </details>
      </div>
    </Card>
  );
}

function SliderField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="grid gap-2">
      <span className="text-[10px] uppercase tracking-[0.24em] text-white/45">
        {label} {formatWeight(value)}
      </span>
      <input
        type="range"
        min={0}
        max={100}
        step={1}
        value={Math.round(value * 100)}
        onChange={(event) => onChange(Number(event.target.value) / 100)}
      />
    </label>
  );
}

export default CampRecommendationsPanel;
