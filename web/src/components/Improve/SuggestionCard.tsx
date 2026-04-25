import React from "react";
import { Card } from "../ui/Card";
import { SeverityLevel } from "./types";

interface SuggestionCardProps {
  title: string;
  severity: SeverityLevel;
  evidence: string;
  deltaValue: number;
  deltaType: string;
  onJumpToLog?: () => void;
  onDismiss?: () => void;
}

const severityColors: Record<SeverityLevel, string> = {
  major: "text-red-400 border-red-400/20 bg-red-950/20",
  average: "text-yellow-400 border-yellow-400/20 bg-yellow-950/20",
  minor: "text-blue-400 border-blue-400/20 bg-blue-950/20",
};

const severityLabels: Record<SeverityLevel, string> = {
  major: "Major",
  average: "Average",
  minor: "Minor",
};

export const SuggestionCard: React.FC<SuggestionCardProps> = ({
  title,
  severity,
  evidence,
  deltaValue,
  deltaType,
  onJumpToLog,
  onDismiss,
}) => {
  const severityStyle = severityColors[severity];

  return (
    <div
      className={`border rounded p-3 sm:p-4 ${severityStyle} transition-all hover:border-white/30`}
    >
      <div className="flex items-start justify-between gap-3 mb-2">
        <div className="flex-1">
          <div className="flex items-center gap-2">
            <span
              className={`text-xs font-bold ${severityStyle.split(" ")[0]}`}
            >
              {severityLabels[severity]}
            </span>
            <h4 className="font-archaic text-sm text-white">{title}</h4>
          </div>
        </div>

        {onDismiss && (
          <button
            onClick={onDismiss}
            className="text-white/40 hover:text-white/60 transition-colors text-lg leading-none"
            title="Dismiss suggestion"
          >
            ✕
          </button>
        )}
      </div>

      <p className="text-xs text-white/70 mb-2">{evidence}</p>

      <div className="flex items-center justify-between gap-2 text-xs">
        <span className="text-white/60">
          {deltaValue > 0 ? "+" : ""}
          {deltaValue.toFixed(1)}% {deltaType}
        </span>
        {onJumpToLog && (
          <button
            onClick={onJumpToLog}
            className="text-blue-400 hover:text-blue-300 transition-colors underline"
          >
            Jump to log
          </button>
        )}
      </div>
    </div>
  );
};

SuggestionCard.displayName = "SuggestionCard";
