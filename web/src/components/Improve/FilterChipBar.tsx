import React from "react";

interface FilterChipBarProps {
  characters?: string[];
  abilities?: string[];
  selectedCharacter?: string;
  selectedAbility?: string;
  onCharacterChange?: (char: string | undefined) => void;
  onAbilityChange?: (ability: string | undefined) => void;
}

export const FilterChipBar: React.FC<FilterChipBarProps> = ({
  characters = [],
  abilities = [],
  selectedCharacter,
  selectedAbility,
  onCharacterChange,
  onAbilityChange,
}) => {
  return (
    <div className="flex flex-wrap gap-2 text-xs">
      {characters.length > 0 && (
        <div className="flex gap-1 items-center">
          <span className="text-white/50">Characters:</span>
          {characters.map((char) => (
            <button
              key={char}
              onClick={() =>
                onCharacterChange?.(
                  selectedCharacter === char ? undefined : char,
                )
              }
              className={`px-2 py-1 rounded border transition-all ${
                selectedCharacter === char
                  ? "border-white/40 bg-white/10 text-white"
                  : "border-white/10 text-white/60 hover:border-white/20"
              }`}
            >
              {char}
            </button>
          ))}
        </div>
      )}

      {abilities.length > 0 && (
        <div className="flex gap-1 items-center">
          <span className="text-white/50">Abilities:</span>
          {abilities.map((ability) => (
            <button
              key={ability}
              onClick={() =>
                onAbilityChange?.(
                  selectedAbility === ability ? undefined : ability,
                )
              }
              className={`px-2 py-1 rounded border transition-all ${
                selectedAbility === ability
                  ? "border-white/40 bg-white/10 text-white"
                  : "border-white/10 text-white/60 hover:border-white/20"
              }`}
            >
              {ability}
            </button>
          ))}
        </div>
      )}
    </div>
  );
};

FilterChipBar.displayName = "FilterChipBar";
