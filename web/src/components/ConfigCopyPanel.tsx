import { useEffect, useMemo, useState } from "react";
import { CheckCircle, Copy, Info, WarningCircle } from "@phosphor-icons/react";
import type { CharacterConfig } from "../types";

type CopySubset = "class_params" | "rotation" | "both";

type CopyStatus = "success" | "error";

interface ConfigCopyResponseEntry {
  char: string;
  status: CopyStatus;
  diff_summary: string;
}

interface ConfigCopyPanelProps {
  characterConfigs: CharacterConfig[];
}

function parseCopyResponse(payload: unknown): ConfigCopyResponseEntry[] {
  if (Array.isArray(payload)) {
    return payload.map((entry) =>
      entry && typeof entry === "object"
        ? {
            char: String((entry as { char?: unknown }).char ?? ""),
            status: ((entry as { status?: unknown }).status as CopyStatus) ?? "error",
            diff_summary: String((entry as { diff_summary?: unknown }).diff_summary ?? ""),
          }
        : {
            char: "",
            status: "error",
            diff_summary: "Malformed response entry",
          },
    );
  }

  if (
    payload &&
    typeof payload === "object" &&
    Array.isArray((payload as { results?: unknown }).results)
  ) {
    return parseCopyResponse((payload as { results: unknown }).results);
  }

  return [];
}

function copySubsetNeedsRotation(subset: CopySubset): boolean {
  return subset === "rotation" || subset === "both";
}

export default function ConfigCopyPanel({ characterConfigs }: ConfigCopyPanelProps) {
  const [fromChar, setFromChar] = useState("");
  const [toChars, setToChars] = useState<string[]>([]);
  const [subset, setSubset] = useState<CopySubset>("both");
  const [loading, setLoading] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [toastType, setToastType] = useState<"ok" | "err">("ok");
  const [results, setResults] = useState<ConfigCopyResponseEntry[]>([]);

  const sourceCharacter = characterConfigs.find(
    (character) => character.character_name === fromChar,
  );

  const targetOptions = useMemo(() => {
    const targetCandidates = characterConfigs.filter(
      (character) => character.character_name !== fromChar,
    );

    if (!sourceCharacter || subset === "class_params") {
      return targetCandidates;
    }

    return targetCandidates.filter(
      (character) =>
        character.class.toLowerCase() === sourceCharacter.class.toLowerCase(),
    );
  }, [characterConfigs, fromChar, subset, sourceCharacter]);

  const selectedSourceClass = sourceCharacter?.class;

  useEffect(() => {
    if (toChars.length === 0) return;

    const validToChars = new Set(targetOptions.map((character) => character.character_name));
    const next = toChars.filter((value) => validToChars.has(value));

    if (next.length !== toChars.length) {
      setToChars(next);
    }
  }, [targetOptions, toChars]);

  const hasTargets = toChars.length > 0;

  async function handleCopy() {
    if (!sourceCharacter || toChars.length === 0) return;

    setLoading(true);
    setToast(null);

    try {
      const response = await fetch("/api/config/copy", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          from_char: fromChar,
          to_chars: toChars,
          subset,
        }),
      });

      if (response.status === 404 || response.status === 501) {
        setToast("Config copy backend unavailable (demo mode)");
        setToastType("err");
        setLoading(false);
        return;
      }

      const payload = await response.json();
      const copyResults = parseCopyResponse(payload);
      setResults(copyResults);

      if (!response.ok) {
        const message =
          copyResults.length > 0
            ? copyResults.map((result) => `${result.char}: ${result.diff_summary}`).join(" | ")
            : `Copy request failed with HTTP ${response.status}`;
        setToast(message);
        setToastType("err");
        return;
      }

      if (copyResults.every((result) => result.status === "success")) {
        setToast("Copy completed successfully.");
        setToastType("ok");
      } else if (copyResults.every((result) => result.status === "error")) {
        setToast("Copy failed for all targets.");
        setToastType("err");
      } else {
        setToast("Copy completed with mixed results.");
        setToastType("ok");
      }
    } catch (error) {
      setToast(error instanceof Error ? error.message : "Copy request failed");
      setToastType("err");
    } finally {
      setLoading(false);
    }
  }

  function toggleTarget(characterName: string) {
    setToChars((prev) =>
      prev.includes(characterName)
        ? prev.filter((entry) => entry !== characterName)
        : [...prev, characterName],
    );
  }

  return (
    <section className="rounded border border-magentaglow/30 bg-violet/20 p-4 mb-6">
      <h3 className="font-archaic text-base text-white uppercase tracking-wide mb-4 flex items-center gap-2">
        <Copy size={16} className="text-magentaglow" />
        Config Copy
      </h3>

      <div className="grid gap-4 md:grid-cols-2">
        <label className="flex flex-col gap-2 text-xs font-tech text-white/70">
          Source Character
          <div className="flex items-center gap-2">
            <input
              type="text"
              list="config-copy-source"
              placeholder={sourceCharacter ? sourceCharacter.character_name : "Search source character"}
              value={fromChar}
              onChange={(event) => {
                setFromChar(event.target.value);
                setResults([]);
              }}
              className="w-full bg-void border border-white/20 text-white text-xs px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
            />
            {sourceCharacter && (
              <span className="text-[10px] uppercase tracking-widest text-white/45 whitespace-nowrap">
                {selectedSourceClass}
              </span>
            )}
          </div>
          <datalist id="config-copy-source">
            {characterConfigs.map((character) => (
              <option
                key={character.character_name}
                value={character.character_name}
                label={`${character.character_name} (${character.class})`}
              />
            ))}
          </datalist>
        </label>

        <label className="flex flex-col gap-2 text-xs font-tech text-white/70">
          Target List ({targetOptions.length} available)
          <div className="max-h-40 overflow-y-auto border border-white/10 bg-void/70 p-2">
            {targetOptions.length === 0 ? (
              <p className="text-[10px] text-white/50 font-rune">
                {copySubsetNeedsRotation(subset) && fromChar
                  ? "No matching-class targets available for rotation copy."
                  : "No valid targets.")}
              </p>
            ) : (
              targetOptions.map((character) => {
                const isChecked = toChars.includes(character.character_name);
                return (
                  <label
                    key={character.character_name}
                    className={`flex items-center justify-between px-2 py-1.5 border rounded-sm cursor-pointer text-xs transition-colors ${
                      isChecked
                        ? "border-magentaglow/50 bg-magentaglow/10"
                        : "border-white/10 hover:border-magentaglow/40"
                    }`}
                  >
                    <span className="text-white/80 font-rune">
                      {character.character_name}
                    </span>
                    <span className="text-[10px] uppercase tracking-widest text-white/50">
                      {character.class}
                    </span>
                    <input
                      type="checkbox"
                      checked={isChecked}
                      onChange={() => toggleTarget(character.character_name)}
                      className="accent-magentaglow ml-3"
                    />
                  </label>
                );
              })
            )}
          </div>
        </label>
      </div>

      <fieldset className="mt-4">
        <legend className="text-xs font-tech uppercase tracking-[0.24em] text-white/50 mb-2">
          Subset
        </legend>
        <div className="grid grid-cols-3 gap-2 text-xs font-tech">
          <label className="flex items-center gap-2 p-2 border border-white/10 bg-void/60 cursor-pointer">
            <input
              type="radio"
              checked={subset === "class_params"}
              onChange={() => setSubset("class_params")}
            />
            <span>Class Params Only</span>
          </label>
          <label className="flex items-center gap-2 p-2 border border-white/10 bg-void/60 cursor-pointer">
            <input
              type="radio"
              checked={subset === "rotation"}
              onChange={() => setSubset("rotation")}
            />
            <span>Rotation Only</span>
          </label>
          <label className="flex items-center gap-2 p-2 border border-white/10 bg-void/60 cursor-pointer">
            <input
              type="radio"
              checked={subset === "both"}
              onChange={() => setSubset("both")}
            />
            <span>Both</span>
          </label>
        </div>
      </fieldset>

      <div className="mt-4 flex items-center justify-between">
        <button
          type="button"
          onClick={handleCopy}
          disabled={loading || !fromChar || !hasTargets || characterConfigs.length === 0}
          className="px-4 py-2 bg-magentadark/20 border border-magentaglow text-white text-xs font-tech uppercase tracking-[0.22em] hover:bg-magentadark/40 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {loading ? "Copying…" : "Copy"}
        </button>

        <div className="flex items-center gap-2 text-[10px] font-rune text-white/60">
          <Info size={12} />
          <span>
            {sourceCharacter
              ? copySubsetNeedsRotation(subset)
                ? "Rotation requires source-target class match"
                : "Class-only copy can target any character"
              : "Choose source first"}
          </span>
        </div>
      </div>

      {toast && (
        <div
          className={`mt-4 border px-3 py-2 text-xs font-rune ${
            toastType === "ok"
              ? "border-emerald-400/30 bg-emerald-500/10 text-emerald-200"
              : "border-rose-400/30 bg-rose-500/10 text-rose-200"
          }`}
        >
          {toast}
        </div>
      )}

      {results.length > 0 && (
        <div className="mt-4 border border-white/10 bg-void/40">
          <div className="px-3 py-2 border-b border-white/10 text-[10px] uppercase tracking-[0.24em] text-white/60 font-rune">
            Copy Results
          </div>
          <ul className="divide-y divide-white/10">
            {results.map((result) => (
              <li
                key={`${result.char}-${result.status}`}
                className="flex items-start gap-2 px-3 py-2 text-xs"
              >
                {result.status === "success" ? (
                  <CheckCircle
                    size={14}
                    className="text-green-400 shrink-0 mt-0.5"
                  />
                ) : (
                  <WarningCircle
                    size={14}
                    className="text-rose-400 shrink-0 mt-0.5"
                  />
                )}
                <span className="font-tech text-white/90">{result.char}</span>
                <span className="text-white/60">{result.diff_summary}</span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
