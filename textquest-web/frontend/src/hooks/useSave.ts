import { useState } from "react";
import { api } from "../lib/api.ts";

export type SaveState = "idle" | "saving" | "saved" | "error";

export interface UseSaveResult<TBody> {
  state: SaveState;
  error: string | null;
  save: (body: TBody) => Promise<void>;
}

export function useSave<TBody>(method: "PUT" | "POST", path: string): UseSaveResult<TBody> {
  const [state, setState] = useState<SaveState>("idle");
  const [error, setError] = useState<string | null>(null);

  const save = async (body: TBody) => {
    setState("saving");
    setError(null);
    try {
      if (method === "PUT") {
        await api.put(path, body);
      } else {
        await api.post(path, body);
      }
      setState("saved");
      setTimeout(() => setState("idle"), 1500);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setState("error");
    }
  };

  return { state, error, save };
}
