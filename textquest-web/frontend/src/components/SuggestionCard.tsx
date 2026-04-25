import { Button } from "./Button";
import { Card } from "./Card";
import type { Suggestion } from "../types/suggestion";
import { AlertCircle, Check, Clock, X } from "lucide-react";

interface SuggestionCardProps {
  suggestion: Suggestion;
  onAccept: (id: string) => void;
  onDismiss: (id: string) => void;
  onDefer: (id: string) => void;
  disabled?: boolean;
}

const tierLabel: Record<1 | 2 | 3, string> = {
  1: "Priority",
  2: "Recommended",
  3: "Optional",
};

const tierColor: Record<1 | 2 | 3, "danger" | "warn" | "info"> = {
  1: "danger",
  2: "warn",
  3: "info",
};

export function SuggestionCard({
  suggestion,
  onAccept,
  onDismiss,
  onDefer,
  disabled = false,
}: SuggestionCardProps) {
  const isResolved = suggestion.status !== "pending";

  return (
    <Card
      accent={tierColor[suggestion.tier]}
      title={`Tier ${suggestion.tier} — ${tierLabel[suggestion.tier]}`}
      icon={<AlertCircle className="w-4 h-4" />}
    >
      <div className="space-y-3">
        <div>
          <h3 className="text-sm font-semibold text-neriak-text mb-1">{suggestion.title}</h3>
          <p className="text-xs text-neriak-muted mb-2">{suggestion.description}</p>
        </div>

        {Object.keys(suggestion.data).length > 0 && (
          <div className="bg-bg-secondary rounded p-2 text-[11px] font-mono text-neriak-muted max-h-32 overflow-auto">
            <dl className="space-y-1">
              {Object.entries(suggestion.data).map(([key, value]) => (
                <div key={key} className="flex justify-between gap-2">
                  <dt className="text-neriak-dim flex-shrink-0">{key}:</dt>
                  <dd className="text-neriak-text text-right flex-shrink">{String(value)}</dd>
                </div>
              ))}
            </dl>
          </div>
        )}

        {isResolved && (
          <div className="text-xs text-neriak-muted p-2 bg-bg-secondary rounded">
            Status: <span className="capitalize font-semibold">{suggestion.status}</span>
          </div>
        )}

        {!isResolved && (
          <div className="flex gap-2 pt-2">
            <Button
              variant="primary"
              onClick={() => onAccept(suggestion.id)}
              disabled={disabled}
              className="flex-1 text-xs py-1 h-8"
            >
              <Check className="w-3 h-3 mr-1 inline" />
              Accept
            </Button>
            <Button
              variant="secondary"
              onClick={() => onDefer(suggestion.id)}
              disabled={disabled}
              className="flex-1 text-xs py-1 h-8"
            >
              <Clock className="w-3 h-3 mr-1 inline" />
              Defer
            </Button>
            <Button
              variant="danger"
              onClick={() => onDismiss(suggestion.id)}
              disabled={disabled}
              className="flex-1 text-xs py-1 h-8"
            >
              <X className="w-3 h-3 mr-1 inline" />
              Dismiss
            </Button>
          </div>
        )}
      </div>
    </Card>
  );
}
