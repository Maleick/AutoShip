import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { DashboardActionRequest, DashboardSnapshot } from "../dashboard";
import SpawnFinderPanel from "./SpawnFinderPanel";

const spawnFinder: DashboardSnapshot["spawnFinder"] = {
  observers: [
    {
      clientId: 7,
      characterName: "Frostreaver",
      zone: "Plane of Fire",
      totalSpawns: 2,
    },
  ],
  items: [
    {
      observerClientId: 7,
      observerName: "Frostreaver",
      observerZone: "Plane of Fire",
      spawnId: 1001,
      name: "a lava walker",
      spawnType: "NPC",
      level: 60,
      className: "MNK",
      raceName: "Human",
      distance: 42,
      hpPct: 100,
      isCurrentTarget: false,
    },
    {
      observerClientId: 7,
      observerName: "Frostreaver",
      observerZone: "Plane of Fire",
      spawnId: 1002,
      name: "a fire giant",
      spawnType: "NPC",
      level: 61,
      className: "WAR",
      raceName: "Ogre",
      distance: 18,
      hpPct: 82,
      isCurrentTarget: true,
    },
  ],
};

describe("SpawnFinderPanel", () => {
  it("filters, sorts, and targets spawns", async () => {
    const submitAction = vi.fn<(_: DashboardActionRequest) => Promise<unknown>>(
      async () => undefined
    );

    render(<SpawnFinderPanel spawnFinder={spawnFinder} submitAction={submitAction} />);

    expect(screen.getByText("a fire giant")).toBeInTheDocument();
    expect(screen.getByText("Current Target")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Spawn sort"), {
      target: { value: "name" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Ascending" }));

    const rows = screen.getAllByText(/a (fire giant|lava walker)/);
    expect(rows[0]).toHaveTextContent("a lava walker");

    fireEvent.change(screen.getByPlaceholderText(/Filter by name, class, race/i), {
      target: { value: "ogre" },
    });

    await waitFor(() => {
      expect(screen.getByText("a fire giant")).toBeInTheDocument();
      expect(screen.queryByText("a lava walker")).not.toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "Target" }));

    await waitFor(() => {
      expect(submitAction).toHaveBeenCalledWith({
        type: "target_spawn",
        client_id: 7,
        spawn_id: 1002,
      });
    });
  });
});
