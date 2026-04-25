import React from "react";

interface SeverityToggleProps {
  showMinor: boolean;
  onToggle: (show: boolean) => void;
  minorCount?: number;
}

export const SeverityToggle: React.FC<SeverityToggleProps> = ({
  showMinor,
  onToggle,
  minorCount = 0,
}) => {
  return (
    <div className="flex items-center gap-2 text-xs text-white/60">
      <label className="flex items-center gap-2 cursor-pointer hover:text-white/80 transition-colors">
        <input
          type="checkbox"
          checked={showMinor}
          onChange={(e) => onToggle(e.target.checked)}
          className="w-3 h-3"
        />
        <span>Show minor tips</span>
        {minorCount > 0 && (
          <span className="text-white/40">({minorCount})</span>
        )}
      </label>
    </div>
  );
};

SeverityToggle.displayName = "SeverityToggle";
