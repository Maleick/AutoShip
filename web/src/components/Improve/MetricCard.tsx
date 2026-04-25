import React from "react";
import { Card } from "../ui/Card";
import { formatMetric } from "./utils";

interface MetricCardProps {
  label: string;
  current: number;
  personalBest?: number;
  delta?: number; // percentage
  isWithinNoise?: boolean;
  metricType: "dps" | "healing" | "xphour" | "percent";
  subtitle?: string;
}

export const MetricCard: React.FC<MetricCardProps> = ({
  label,
  current,
  personalBest,
  delta,
  isWithinNoise = false,
  metricType,
  subtitle,
}) => {
  const currentFormatted = formatMetric(current, metricType);
  const deltaFormatted =
    delta !== undefined && delta !== null
      ? `${delta > 0 ? "+" : ""}${delta.toFixed(1)}%`
      : null;

  return (
    <Card
      title={label}
      subtitle={subtitle}
      variant="outlined"
      className="min-w-[200px]"
    >
      <div className="flex flex-col gap-3">
        <div className="text-2xl font-bold text-white">{currentFormatted}</div>

        {personalBest !== undefined && (
          <div className="text-xs text-white/60">
            <div>Personal best: {formatMetric(personalBest, metricType)}</div>
            {deltaFormatted && (
              <div
                className={`mt-1 ${
                  isWithinNoise
                    ? "text-white/40"
                    : delta! > 0
                      ? "text-green-400"
                      : "text-orange-400"
                }`}
              >
                {isWithinNoise ? "≈ " : ""}
                {deltaFormatted}
                {isWithinNoise && " (within noise floor)"}
              </div>
            )}
          </div>
        )}
      </div>
    </Card>
  );
};

MetricCard.displayName = "MetricCard";
