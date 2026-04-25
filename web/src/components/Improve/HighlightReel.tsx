import React from "react";
import { Card } from "../ui/Card";

export interface HighlightMoment {
  id: string;
  title: string;
  type: "win" | "loss" | "critical";
  timestamp: number;
  description: string;
  onJumpToLog?: () => void;
}

interface HighlightReelProps {
  moments: HighlightMoment[];
  onMomentClick?: (moment: HighlightMoment) => void;
}

const typeColors = {
  win: "border-green-400/30 bg-green-950/30",
  loss: "border-red-400/30 bg-red-950/30",
  critical: "border-yellow-400/30 bg-yellow-950/30",
};

const typeIcons = {
  win: "★",
  loss: "!",
  critical: "⚠",
};

export const HighlightReel: React.FC<HighlightReelProps> = ({
  moments,
  onMomentClick,
}) => {
  if (moments.length === 0) {
    return (
      <Card title="Highlights" subtitle="Auto-flagged moments">
        <p className="text-xs text-white/60">No notable moments recorded.</p>
      </Card>
    );
  }

  return (
    <Card title="Highlights" subtitle="Auto-flagged moments">
      <div className="flex gap-2 overflow-x-auto pb-2">
        {moments.map((moment) => (
          <button
            key={moment.id}
            onClick={() => {
              onMomentClick?.(moment);
              moment.onJumpToLog?.();
            }}
            className={`flex-shrink-0 border rounded p-3 min-w-[200px] transition-all hover:border-white/40 ${typeColors[moment.type]}`}
          >
            <div className="flex items-start gap-2">
              <span className="text-lg flex-shrink-0">
                {typeIcons[moment.type]}
              </span>
              <div className="flex-1 text-left">
                <h4 className="text-xs font-archaic text-white">
                  {moment.title}
                </h4>
                <p className="text-[10px] text-white/60 mt-1">
                  {moment.description}
                </p>
                <p className="text-[10px] text-white/40 mt-2">
                  {(moment.timestamp / 1000).toFixed(1)}s
                </p>
              </div>
            </div>
          </button>
        ))}
      </div>
    </Card>
  );
};

HighlightReel.displayName = "HighlightReel";
