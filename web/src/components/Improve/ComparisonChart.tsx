import React from "react";
import { Card } from "../ui/Card";
import { ComparisonPoint } from "./types";

interface ComparisonChartProps {
  points: ComparisonPoint[];
  title?: string;
  ylabel?: string;
}

export const ComparisonChart: React.FC<ComparisonChartProps> = ({
  points,
  title = "Session Comparison",
  ylabel = "Value",
}) => {
  if (points.length === 0) {
    return (
      <Card title={title} variant="outlined">
        <p className="text-xs text-white/60">No data to compare.</p>
      </Card>
    );
  }

  const maxValue = Math.max(
    ...points.map((p) => Math.max(p.current, p.personal_best)),
    1,
  );

  return (
    <Card title={title} variant="outlined">
      <div className="space-y-4">
        {points.map((point, idx) => {
          const currentPct = (point.current / maxValue) * 100;
          const pbPct = (point.personal_best / maxValue) * 100;

          return (
            <div key={idx} className="space-y-1">
              <div className="flex items-center justify-between text-xs">
                <span className="text-white/70">{point.label}</span>
                <div className="flex gap-2 text-white/60">
                  <span>
                    Current:{" "}
                    <span className="text-white">
                      {point.current.toFixed(1)}
                    </span>
                  </span>
                  <span>
                    Best:{" "}
                    <span className="text-white">
                      {point.personal_best.toFixed(1)}
                    </span>
                  </span>
                </div>
              </div>

              <div className="flex gap-2 items-end h-16">
                <div className="flex-1 flex flex-col items-start">
                  <div className="w-full bg-white/10 rounded overflow-hidden h-12">
                    <div
                      className="h-full bg-blue-500/40 transition-all"
                      style={{ width: `${currentPct}%` }}
                    />
                  </div>
                  <span className="text-[10px] text-white/40 mt-1">
                    current
                  </span>
                </div>

                <div className="flex-1 flex flex-col items-start">
                  <div className="w-full bg-white/10 rounded overflow-hidden h-12">
                    <div
                      className="h-full bg-green-600/40 transition-all"
                      style={{ width: `${pbPct}%` }}
                    />
                  </div>
                  <span className="text-[10px] text-white/40 mt-1">
                    personal best
                  </span>
                </div>
              </div>

              {!point.isWithinNoise && (
                <div
                  className={`text-[10px] ${
                    point.delta > 0 ? "text-green-400" : "text-orange-400"
                  }`}
                >
                  {point.delta > 0 ? "+" : ""}
                  {point.delta.toFixed(1)}%
                </div>
              )}
              {point.isWithinNoise && (
                <div className="text-[10px] text-white/40">
                  ≈ within noise floor
                </div>
              )}
            </div>
          );
        })}
      </div>
    </Card>
  );
};

ComparisonChart.displayName = "ComparisonChart";
