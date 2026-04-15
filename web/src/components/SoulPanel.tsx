import {
  Brain,
  Smiley,
  SmileyMeh,
  SmileySad,
  SmileyNervous,
  SmileyWink,
} from "@phosphor-icons/react";

// ── Types ─────────────────────────────────────────────────────────────────────

type Mood =
  | "content"
  | "anxious"
  | "focused"
  | "bored"
  | "excited"
  | "melancholic";

interface SoulState {
  character_id: string;
  mood: Mood;
  personality_traits: string[];
  memory_count: number;
  last_event: string | null;
}

// ── Demo data (stub — no live Soul Engine required) ───────────────────────────

const DEMO_SOUL_STATES: SoulState[] = [
  {
    character_id: "Frostreaver",
    mood: "focused",
    personality_traits: ["cautious", "loyal", "stoic"],
    memory_count: 142,
    last_event: "Recalled the time we cleared Lower Guk in under an hour.",
  },
  {
    character_id: "Shadowdancer",
    mood: "excited",
    personality_traits: ["bold", "mischievous", "curious"],
    memory_count: 87,
    last_event: "Spotted a rare spawn — flagged Maestro of Rancor.",
  },
  {
    character_id: "Ironclad",
    mood: "content",
    personality_traits: ["disciplined", "protective"],
    memory_count: 201,
    last_event: "Completed a successful CH chain rotation.",
  },
  {
    character_id: "Lightbringer",
    mood: "anxious",
    personality_traits: ["empathetic", "nervous", "devout"],
    memory_count: 65,
    last_event: "Mana reserves dropped below sit threshold during combat.",
  },
  {
    character_id: "Aelrindel",
    mood: "bored",
    personality_traits: ["intellectual", "impatient"],
    memory_count: 38,
    last_event: "Waiting on camp cycle — contemplating ice comet upgrades.",
  },
  {
    character_id: "Grok",
    mood: "melancholic",
    personality_traits: ["spiritual", "wise", "brooding"],
    memory_count: 113,
    last_event: "Remembered fallen companions from the Plane of Sky.",
  },
];

// ── Mood config ───────────────────────────────────────────────────────────────

const MOOD_CONFIG: Record<
  Mood,
  { label: string; color: string; borderColor: string; icon: typeof Smiley }
> = {
  content: {
    label: "Content",
    color: "text-cyan-400",
    borderColor: "border-cyan-400/40",
    icon: Smiley,
  },
  anxious: {
    label: "Anxious",
    color: "text-yellow-400",
    borderColor: "border-yellow-400/40",
    icon: SmileyNervous,
  },
  focused: {
    label: "Focused",
    color: "text-spectral",
    borderColor: "border-spectral/40",
    icon: SmileyWink,
  },
  bored: {
    label: "Bored",
    color: "text-white/40",
    borderColor: "border-white/20",
    icon: SmileyMeh,
  },
  excited: {
    label: "Excited",
    color: "text-magentaglow",
    borderColor: "border-magentaglow/40",
    icon: Smiley,
  },
  melancholic: {
    label: "Melancholic",
    color: "text-violet",
    borderColor: "border-violet/40",
    icon: SmileySad,
  },
};

// ── Card component ────────────────────────────────────────────────────────────

function SoulCard({ soul }: { soul: SoulState }) {
  const mood = MOOD_CONFIG[soul.mood] ?? MOOD_CONFIG.content;
  const MoodIcon = mood.icon;

  return (
    <div
      className={`bg-violet/10 border ${mood.borderColor} p-4 flex flex-col gap-3 hover:bg-violet/20 transition-colors`}
    >
      {/* Header */}
      <div className="flex items-center justify-between">
        <span className="font-archaic text-sm text-white tracking-wide">
          {soul.character_id}
        </span>
        <div className={`flex items-center gap-1.5 ${mood.color}`}>
          <MoodIcon size={16} weight="fill" />
          <span className="font-rune text-[10px] uppercase tracking-widest">
            {mood.label}
          </span>
        </div>
      </div>

      {/* Personality traits */}
      <div className="flex flex-wrap gap-1">
        {soul.personality_traits.map((trait) => (
          <span
            key={trait}
            className="text-[9px] uppercase tracking-widest px-1.5 py-0.5 border border-white/10 text-white/50 font-rune"
          >
            {trait}
          </span>
        ))}
      </div>

      {/* Memory count */}
      <div className="flex items-center gap-2">
        <Brain size={12} className="text-magentaglow/60 shrink-0" />
        <span className="text-[10px] text-white/40 font-tech">
          {soul.memory_count} memories
        </span>
      </div>

      {/* Last event */}
      {soul.last_event && (
        <p className="text-[10px] text-white/30 italic leading-relaxed border-t border-white/5 pt-2">
          {soul.last_event}
        </p>
      )}
    </div>
  );
}

// ── Main panel ────────────────────────────────────────────────────────────────

export default function SoulPanel() {
  // Stub data — replace with a `useSoulStates()` hook once the API is wired.
  const souls: SoulState[] = DEMO_SOUL_STATES;

  const moodCounts = souls.reduce<Partial<Record<Mood, number>>>((acc, s) => {
    acc[s.mood] = (acc[s.mood] ?? 0) + 1;
    return acc;
  }, {});

  return (
    <div className="flex-1 flex flex-col overflow-hidden min-w-0">
      {/* Panel header */}
      <div className="flex items-center gap-3 mb-6">
        <div className="w-8 h-8 border border-magentaglow/40 bg-magentadark/15 text-magentaglow flex items-center justify-center shrink-0">
          <Brain size={16} weight="fill" />
        </div>
        <div>
          <h2 className="font-archaic text-base text-white">Soul Engine</h2>
          <p className="text-[10px] uppercase tracking-widest text-white/40 font-rune">
            Character personality &amp; memory monitor
          </p>
        </div>

        {/* Mood summary badges */}
        <div className="ml-auto flex items-center gap-2">
          {(Object.entries(moodCounts) as [Mood, number][]).map(
            ([mood, count]) => {
              const cfg = MOOD_CONFIG[mood];
              return (
                <span
                  key={mood}
                  className={`text-[9px] uppercase tracking-widest px-2 py-0.5 border ${cfg.borderColor} ${cfg.color} font-rune`}
                >
                  {count} {cfg.label}
                </span>
              );
            },
          )}
        </div>
      </div>

      {/* Character grid */}
      <div className="flex-1 overflow-y-auto">
        <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-3 gap-4">
          {souls.map((soul) => (
            <SoulCard key={soul.character_id} soul={soul} />
          ))}
        </div>

        {souls.length === 0 && (
          <div className="flex items-center justify-center h-40 text-white/20 font-archaic text-sm">
            No soul states available
          </div>
        )}
      </div>
    </div>
  );
}
