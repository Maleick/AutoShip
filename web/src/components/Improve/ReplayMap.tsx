import React from "react";
import { Card } from "../ui/Card";

interface Position {
  x: number;
  y: number;
  character: string;
  timestamp: number;
}

interface ReplayMapProps {
  positions: Position[];
  zoneWidth?: number;
  zoneHeight?: number;
}

export const ReplayMap: React.FC<ReplayMapProps> = ({
  positions,
  zoneWidth = 1000,
  zoneHeight = 1000,
}) => {
  return (
    <Card
      title="Position Replay (v2)"
      subtitle="Placeholder: 2D top-down animation"
      variant="outlined"
    >
      <div
        className="relative bg-black/50 border border-white/10 rounded overflow-hidden"
        style={{
          aspectRatio: `${zoneWidth} / ${zoneHeight}`,
          maxHeight: "400px",
        }}
      >
        {/* Grid background */}
        <svg
          className="absolute inset-0 w-full h-full"
          style={{ opacity: 0.1 }}
        >
          <defs>
            <pattern
              id="grid"
              width="50"
              height="50"
              patternUnits="userSpaceOnUse"
            >
              <path
                d="M 50 0 L 0 0 0 50"
                fill="none"
                stroke="white"
                strokeWidth="0.5"
              />
            </pattern>
          </defs>
          <rect width="100%" height="100%" fill="url(#grid)" />
        </svg>

        {/* Character positions (final frame) */}
        {positions.length > 0 && (
          <div className="absolute inset-0">
            {[...new Map(positions.map((p) => [p.character, p])).values()].map(
              (pos) => {
                const x = (pos.x / zoneWidth) * 100;
                const y = (pos.y / zoneHeight) * 100;

                return (
                  <div
                    key={pos.character}
                    className="absolute transform -translate-x-1/2 -translate-y-1/2 text-xs text-white/80"
                    style={{
                      left: `${x}%`,
                      top: `${y}%`,
                    }}
                    title={`${pos.character} @ (${pos.x.toFixed(0)}, ${pos.y.toFixed(0)})`}
                  >
                    <div className="w-3 h-3 border border-blue-400 rounded-full" />
                    <span className="text-[8px] whitespace-nowrap ml-1 inline-block">
                      {pos.character}
                    </span>
                  </div>
                );
              },
            )}
          </div>
        )}

        {positions.length === 0 && (
          <div className="absolute inset-0 flex items-center justify-center text-white/40 text-xs">
            No position data available
          </div>
        )}
      </div>
    </Card>
  );
};

ReplayMap.displayName = "ReplayMap";
