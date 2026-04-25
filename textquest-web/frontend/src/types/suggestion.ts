export type SuggestionTier = 1 | 2 | 3;

export interface Suggestion {
  id: string;
  tier: SuggestionTier;
  title: string;
  description: string;
  data: Record<string, unknown>;
  createdAt: string;
  status: "pending" | "accepted" | "dismissed" | "deferred";
}

export interface SuggestionAction {
  type: "accept" | "dismiss" | "defer";
  suggestionId: string;
  deferUntil?: string;
}
