import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import BardSongPanel, {
  makeDefaultConfig,
  DEFAULT_SONGS,
  DEFAULT_INSTRUMENTS,
} from "./BardSongPanel";
import type { BardConfig } from "../types";

const BARD_CONFIG: BardConfig = {
  character_name: "Melodica",
  twist_enabled: true,
  full_rotation_enabled: true,
  instrument_swap_enabled: true,
  songs: DEFAULT_SONGS,
  instruments: DEFAULT_INSTRUMENTS,
};

describe("BardSongPanel", () => {
  let saveFn: ReturnType<typeof vi.fn>;
  beforeEach(() => {
    saveFn = vi.fn().mockResolvedValue(undefined);
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("renders character name and panel title", () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    expect(screen.getByText(/Melodica.*Bard Configuration/)).toBeInTheDocument();
    expect(screen.getByText(/Song scheduling, twist rotation, and instrument swap/)).toBeInTheDocument();
  });

  it("renders all three engine-control toggles as enabled", () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    expect(screen.getByText("Enable Song Twisting")).toBeInTheDocument();
    expect(screen.getByText("Full Rotation (song weaving)")).toBeInTheDocument();
    expect(screen.getByText("Instrument Swap")).toBeInTheDocument();
  });

  it("renders the default song list", () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    expect(screen.getByText("Celestial Clarity")).toBeInTheDocument();
    expect(screen.getByText("Aeon's Harmony")).toBeInTheDocument();
    expect(screen.getByText("Blade Chords")).toBeInTheDocument();
    expect(screen.getByText("Crescendo of the Siren")).toBeInTheDocument();
    expect(screen.getByText("Warless Superbia")).toBeInTheDocument();
  });

  it("shows disabled state for songs with enabled=false", () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    const warless = screen.getByText("Warless Superbia").closest("div");
    expect(warless?.className).toMatch(/opacity-50/);
  });

  it("toggles individual songs", async () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    const checks = screen.getAllByRole("button", { name: /Disable|Enable/i });
    expect(checks.length).toBeGreaterThan(0);
    await act(async () => {
      fireEvent.click(checks[0]);
    });
  });

  it("edits song name inline", async () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    const nameInput = screen.getByDisplayValue("Celestial Clarity") as HTMLInputElement;
    await act(async () => {
      fireEvent.change(nameInput, { target: { value: "New Song Name" } });
    });
    expect(screen.getByDisplayValue("New Song Name")).toBeInTheDocument();
  });

  it("changes song gem number", async () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    const gemInputs = screen.getAllByDisplayValue("1");
    const gemInput = gemInputs.find(
      (el) => (el as HTMLInputElement).type === "number"
    ) as HTMLInputElement | undefined;
    expect(gemInput).toBeDefined();
    await act(async () => {
      fireEvent.change(gemInput!, { target: { value: "7" } });
    });
  });

  it("changes song category", async () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    const selects = screen.getAllByRole("combobox");
    expect(selects.length).toBeGreaterThan(0);
    await act(async () => {
      fireEvent.change(selects[0], { target: { value: "Regen" } });
    });
  });

  it("reorders songs with move-up button", async () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    const moveUpButtons = screen.getAllByTitle("Move up (higher priority)");
    await act(async () => {
      fireEvent.click(moveUpButtons[1]);
    });
    const names = screen.getAllByRole("textbox").map((el) => (el as HTMLInputElement).value);
    expect(names[0]).toBe("Aeon's Harmony");
  });

  it("reorders songs with move-down button", async () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    const moveDownButtons = screen.getAllByTitle("Move down (lower priority)");
    await act(async () => {
      fireEvent.click(moveDownButtons[0]);
    });
    const names = screen.getAllByRole("textbox").map((el) => (el as HTMLInputElement).value);
    expect(names[1]).toBe("Celestial Clarity");
  });

  it("removes a song", async () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    const removeButtons = screen.getAllByTitle("Remove song");
    await act(async () => {
      fireEvent.click(removeButtons[0]);
    });
    expect(screen.queryByText("Celestial Clarity")).not.toBeInTheDocument();
  });

  it("calls onSave with updated config when Commit is clicked", async () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    const nameInput = screen.getByDisplayValue("Celestial Clarity") as HTMLInputElement;
    await act(async () => {
      fireEvent.change(nameInput, { target: { value: "Changed Song" } });
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /Commit/i }));
    });
    await waitFor(() =>
      expect(saveFn).toHaveBeenCalledWith(
        expect.objectContaining({
          songs: expect.arrayContaining([
            expect.objectContaining({ name: "Changed Song" }),
          ]),
        })
      )
    );
  });

  it("shows dirty state indicator when changes are pending", async () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    const nameInput = screen.getByDisplayValue("Celestial Clarity") as HTMLInputElement;
    await act(async () => {
      fireEvent.change(nameInput, { target: { value: "Changed Song" } });
    });
    const discardBtn = screen.getByTitle("Discard changes");
    expect(discardBtn).toBeInTheDocument();
  });

  it("discards changes when discard button is clicked", async () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    const nameInput = screen.getByDisplayValue("Celestial Clarity") as HTMLInputElement;
    await act(async () => {
      fireEvent.change(nameInput, { target: { value: "Changed Song" } });
    });
    await act(async () => {
      fireEvent.click(screen.getByTitle("Discard changes"));
    });
    expect(screen.getByDisplayValue("Celestial Clarity")).toBeInTheDocument();
  });

  it("disables Commit button when no changes are made", () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    const commitBtn = screen.getByRole("button", { name: /Commit/i });
    expect(commitBtn).toBeDisabled();
  });

  it("toggles twist_enabled", async () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    const twistToggle = screen.getByText("Enable Song Twisting").closest("button")!;
    await act(async () => {
      fireEvent.click(twistToggle);
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /Commit/i }));
    });
    expect(saveFn).toHaveBeenCalledWith(
      expect.objectContaining({ twist_enabled: false })
    );
  });

  it("toggles full_rotation_enabled", async () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    const fullRotToggle = screen.getByText("Full Rotation (song weaving)").closest("button")!;
    await act(async () => {
      fireEvent.click(fullRotToggle);
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /Commit/i }));
    });
    expect(saveFn).toHaveBeenCalledWith(
      expect.objectContaining({ full_rotation_enabled: false })
    );
  });

  it("toggles instrument_swap_enabled", async () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    const swapToggle = screen.getByText("Instrument Swap").closest("button")!;
    await act(async () => {
      fireEvent.click(swapToggle);
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /Commit/i }));
    });
    expect(saveFn).toHaveBeenCalledWith(
      expect.objectContaining({ instrument_swap_enabled: false })
    );
  });

  it("shows instrument editor section", () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    expect(screen.getByText(/Instrument Inventory/i)).toBeInTheDocument();
    expect(screen.getByText(/Instrument Set 1/i)).toBeInTheDocument();
    expect(screen.getByText(/String Item ID/i)).toBeInTheDocument();
    expect(screen.getByText(/Brass Item ID/i)).toBeInTheDocument();
    expect(screen.getByText(/Wind Item ID/i)).toBeInTheDocument();
    expect(screen.getByText(/Percussion Item ID/i)).toBeInTheDocument();
  });

  it("adds and removes instrument sets", async () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    expect(screen.getAllByText(/Instrument Set \d/i)).toHaveLength(1);
    await act(async () => {
      fireEvent.click(screen.getByText("Add Instrument Set"));
    });
    expect(screen.getAllByText(/Instrument Set \d/i)).toHaveLength(2);
    const removeButtons = screen.getAllByTitle("Remove set");
    await act(async () => {
      fireEvent.click(removeButtons[0]);
    });
    expect(screen.getAllByText(/Instrument Set \d/i)).toHaveLength(1);
  });

  it("updates instrument item IDs", async () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    const numberInputs = screen.getAllByRole("spinbutton");
    expect(numberInputs.length).toBeGreaterThan(0);
    await act(async () => {
      fireEvent.change(numberInputs[0], { target: { value: "12345" } });
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /Commit/i }));
    });
    expect(saveFn).toHaveBeenCalledWith(
      expect.objectContaining({
        instruments: expect.arrayContaining([
          expect.objectContaining({ string_item_id: 12345 }),
        ]),
      })
    );
  });

  it("shows saved confirmation message", async () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    const nameInput = screen.getByDisplayValue("Celestial Clarity") as HTMLInputElement;
    await act(async () => {
      fireEvent.change(nameInput, { target: { value: "Saved Song" } });
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /Commit/i }));
    });
    await waitFor(() => expect(screen.getByText("Saved")).toBeInTheDocument());
  });

  it("handles save errors gracefully", async () => {
    const failingSave = vi.fn().mockRejectedValue(new Error("Network error"));
    render(<BardSongPanel config={BARD_CONFIG} onSave={failingSave} />);
    const nameInput = screen.getByDisplayValue("Celestial Clarity") as HTMLInputElement;
    await act(async () => {
      fireEvent.change(nameInput, { target: { value: "Error Song" } });
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /Commit/i }));
    });
    await waitFor(() =>
      expect(screen.getByText("Network error")).toBeInTheDocument()
    );
  });

  it("renders drag-to-reorder hint", () => {
    render(<BardSongPanel config={BARD_CONFIG} onSave={saveFn} />);
    expect(screen.getByText(/Drag to reorder/i)).toBeInTheDocument();
  });
});

describe("makeDefaultConfig", () => {
  it("creates a config with all three features enabled", () => {
    const cfg = makeDefaultConfig("TestBard");
    expect(cfg.character_name).toBe("TestBard");
    expect(cfg.twist_enabled).toBe(true);
    expect(cfg.full_rotation_enabled).toBe(true);
    expect(cfg.instrument_swap_enabled).toBe(true);
  });

  it("defaults to 5 combat songs", () => {
    const cfg = makeDefaultConfig("TestBard");
    expect(cfg.songs).toHaveLength(5);
  });

  it("defaults to 1 instrument set", () => {
    const cfg = makeDefaultConfig("TestBard");
    expect(cfg.instruments).toHaveLength(1);
  });

  it("generates unique song IDs", () => {
    const cfg = makeDefaultConfig("TestBard");
    const ids = cfg.songs.map((s) => s.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("marks the last song (Insult) as disabled", () => {
    const cfg = makeDefaultConfig("TestBard");
    expect(cfg.songs[4].enabled).toBe(false);
  });

  it("sets correct categories for all songs", () => {
    const cfg = makeDefaultConfig("TestBard");
    expect(cfg.songs[0].category).toBe("Haste");
    expect(cfg.songs[1].category).toBe("SpellFocus");
    expect(cfg.songs[2].category).toBe("MeleeProc");
    expect(cfg.songs[3].category).toBe("Crescendo");
    expect(cfg.songs[4].category).toBe("Insult");
  });

  it("assigns sequential gem numbers 1-5", () => {
    const cfg = makeDefaultConfig("TestBard");
    expect(cfg.songs.map((s) => s.gem)).toEqual([1, 2, 3, 4, 5]);
  });
});

describe("DEFAULT_SONGS", () => {
  it("has 5 entries", () => {
    expect(DEFAULT_SONGS).toHaveLength(5);
  });

  it("all have valid categories", () => {
    const validCategories = [
      "Haste", "SpellFocus", "MeleeProc", "Crescendo", "Insult",
      "RunSpeed", "Regen", "Tank", "Slow", "Accelerando",
      "Mez", "Dot", "Arcane", "Other",
    ];
    for (const song of DEFAULT_SONGS) {
      expect(validCategories).toContain(song.category);
    }
  });

  it("all have non-null min_recast_ticks", () => {
    for (const song of DEFAULT_SONGS) {
      expect(song.min_recast_ticks).toBeGreaterThan(0);
    }
  });

  it("Insult song has null buff_duration_ticks (no persistent buff)", () => {
    const insult = DEFAULT_SONGS.find((s) => s.category === "Insult");
    expect(insult?.buff_duration_ticks).toBeNull();
  });
});

describe("DEFAULT_INSTRUMENTS", () => {
  it("has 1 empty instrument set", () => {
    expect(DEFAULT_INSTRUMENTS).toHaveLength(1);
    expect(DEFAULT_INSTRUMENTS[0].string_item_id).toBeNull();
    expect(DEFAULT_INSTRUMENTS[0].brass_item_id).toBeNull();
    expect(DEFAULT_INSTRUMENTS[0].wind_item_id).toBeNull();
    expect(DEFAULT_INSTRUMENTS[0].percussion_item_id).toBeNull();
  });
});
