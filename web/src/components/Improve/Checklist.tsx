import React, { useState } from "react";
import { Card } from "../ui/Card";
import { ChecklistItem } from "./types";

interface ChecklistProps {
  items: ChecklistItem[];
  groupByCharacter?: boolean;
}

export const Checklist: React.FC<ChecklistProps> = ({
  items,
  groupByCharacter = true,
}) => {
  const [expandedCharacters, setExpandedCharacters] = useState<Set<string>>(
    new Set(),
  );

  if (items.length === 0) {
    return (
      <Card title="Checklist" variant="outlined">
        <p className="text-xs text-white/60">
          No performance data recorded yet.
        </p>
      </Card>
    );
  }

  const byCharacter = groupByCharacter
    ? items.reduce(
        (acc, item) => {
          if (!acc[item.character]) acc[item.character] = [];
          acc[item.character].push(item);
          return acc;
        },
        {} as Record<string, ChecklistItem[]>,
      )
    : { All: items };

  const toggleCharacter = (char: string) => {
    const next = new Set(expandedCharacters);
    if (next.has(char)) next.delete(char);
    else next.add(char);
    setExpandedCharacters(next);
  };

  return (
    <Card title="Checklist: What Went Well" variant="outlined">
      <div className="space-y-2">
        {Object.entries(byCharacter).map(([character, charItems]) => (
          <div key={character}>
            {groupByCharacter && (
              <button
                onClick={() => toggleCharacter(character)}
                className="w-full flex items-center justify-between gap-2 px-3 py-2 border border-white/10 rounded hover:border-white/20 transition-all"
              >
                <span className="text-xs font-archaic text-white">
                  {character}
                </span>
                <span className="text-xs text-white/60">
                  {charItems.filter((i) => i.passed).length}/{charItems.length}
                </span>
              </button>
            )}

            {(!groupByCharacter || expandedCharacters.has(character)) && (
              <div className="ml-2 mt-2 space-y-1 border-l border-white/10 pl-3">
                {charItems.map((item) => (
                  <div
                    key={item.id}
                    className={`flex items-start gap-2 text-xs py-1 ${
                      item.passed ? "text-green-400" : "text-yellow-400"
                    }`}
                  >
                    <span className="flex-shrink-0 mt-0.5">
                      {item.passed ? "✓" : "◐"}
                    </span>
                    <div className="flex-1">
                      <p>{item.description}</p>
                      {item.progress !== undefined && (
                        <div className="mt-1 h-1 bg-white/10 rounded overflow-hidden">
                          <div
                            className={`h-full ${
                              item.passed ? "bg-green-600" : "bg-yellow-600"
                            }`}
                            style={{ width: `${item.progress}%` }}
                          />
                        </div>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </Card>
  );
};

Checklist.displayName = "Checklist";
