import React from "react";
import { SessionCampFingerprint } from "./types";

interface CampPickerProps {
  camps: SessionCampFingerprint[];
  selected?: SessionCampFingerprint;
  onSelect: (camp: SessionCampFingerprint | undefined) => void;
}

export const CampPicker: React.FC<CampPickerProps> = ({
  camps,
  selected,
  onSelect,
}) => {
  return (
    <div className="flex flex-col gap-2">
      <label className="text-xs font-archaic text-white/80">Camp Filter</label>
      <div className="flex flex-wrap gap-2">
        <button
          onClick={() => onSelect(undefined)}
          className={`px-3 py-1 rounded border text-xs transition-all ${
            !selected
              ? "border-white/40 bg-white/10 text-white"
              : "border-white/10 text-white/60 hover:border-white/20"
          }`}
        >
          All camps
        </button>
        {camps.map((camp) => (
          <button
            key={camp.campId}
            onClick={() => onSelect(camp)}
            title={`${camp.zone} - ${camp.mobSet.join(", ")}`}
            className={`px-3 py-1 rounded border text-xs transition-all ${
              selected?.campId === camp.campId
                ? "border-white/40 bg-white/10 text-white"
                : "border-white/10 text-white/60 hover:border-white/20"
            }`}
          >
            {camp.zone}
          </button>
        ))}
      </div>
    </div>
  );
};

CampPicker.displayName = "CampPicker";
