import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import ImprovePanel from "./ImprovePanel";

describe("ImprovePanel", () => {
  beforeEach(() => {
    // Mock fetch
    global.fetch = vi.fn();
    // Mock import.meta.env
    Object.defineProperty(import.meta, "env", {
      value: { VITE_API_TOKEN: "test-token" },
    });
  });

  it("renders character selector", () => {
    render(<ImprovePanel />);
    expect(
      screen.getByPlaceholderText("Enter character name"),
    ).toBeInTheDocument();
  });

  it("shows loading state when character is selected", async () => {
    (global.fetch as any).mockResolvedValueOnce({
      ok: false,
      status: 404,
    });

    render(<ImprovePanel />);
    const input = screen.getByPlaceholderText("Enter character name");

    fireEvent.change(input, { target: { value: "Teek" } });

    await waitFor(() => {
      expect(screen.getByText(/No closed sessions/i)).toBeInTheDocument();
    });
  });

  it("fetches and displays session debrief", async () => {
    const mockDebrief = {
      session_id: "sess-123",
      character_id: "Teek",
      started_at: new Date().toISOString(),
      ended_at: new Date().toISOString(),
      top_wins: ["kills/hour up 15%"],
      top_losses: ["3 stuck events"],
      suggestions: [],
    };

    (global.fetch as any).mockResolvedValueOnce({
      ok: true,
      json: async () => mockDebrief,
    });

    render(<ImprovePanel />);
    const input = screen.getByPlaceholderText("Enter character name");

    fireEvent.change(input, { target: { value: "Teek" } });

    await waitFor(() => {
      expect(screen.getByText("sess-123")).toBeInTheDocument();
    });
  });

  it("renders suggestion list with action buttons", async () => {
    const mockDebrief = {
      session_id: "sess-123",
      character_id: "Teek",
      started_at: new Date().toISOString(),
      ended_at: new Date().toISOString(),
      top_wins: ["kills/hour up 15%"],
      top_losses: ["3 stuck events"],
      suggestions: [
        {
          id: "sugg-1",
          title: "increase_mana_threshold",
          current_value: "20",
          proposed_value: "25",
          rationale: "Mana drops in long fights",
          confidence: 0.85,
          config_key: "combat.mana_threshold",
        },
      ],
    };

    (global.fetch as any).mockResolvedValueOnce({
      ok: true,
      json: async () => mockDebrief,
    });

    render(<ImprovePanel />);
    const input = screen.getByPlaceholderText("Enter character name");

    fireEvent.change(input, { target: { value: "Teek" } });

    await waitFor(() => {
      expect(screen.getByText("increase_mana_threshold")).toBeInTheDocument();
      expect(screen.getByText(/20 → 25/)).toBeInTheDocument();
    });

    // Check for action buttons
    expect(screen.getByText(/Accept/i)).toBeInTheDocument();
    expect(screen.getByText(/Reject/i)).toBeInTheDocument();
    expect(screen.getByText(/Snooze/i)).toBeInTheDocument();
    expect(screen.getByText(/Global/i)).toBeInTheDocument();
  });

  it("calls accept endpoint when Accept button is clicked", async () => {
    const mockDebrief = {
      session_id: "sess-123",
      character_id: "Teek",
      started_at: new Date().toISOString(),
      ended_at: new Date().toISOString(),
      top_wins: [],
      top_losses: [],
      suggestions: [
        {
          id: "sugg-1",
          title: "test_suggestion",
          current_value: "10",
          proposed_value: "20",
          rationale: "Test rationale",
          confidence: 0.9,
          config_key: "test.key",
        },
      ],
    };

    (global.fetch as any)
      .mockResolvedValueOnce({
        ok: true,
        json: async () => mockDebrief,
      })
      .mockResolvedValueOnce({
        ok: true,
        json: async () => ({ success: true }),
      })
      .mockResolvedValueOnce({
        ok: true,
        json: async () => mockDebrief,
      });

    render(<ImprovePanel />);
    const input = screen.getByPlaceholderText("Enter character name");

    fireEvent.change(input, { target: { value: "Teek" } });

    await waitFor(() => {
      expect(screen.getByText("test_suggestion")).toBeInTheDocument();
    });

    const acceptButton = screen.getByText(/Accept/i);
    fireEvent.click(acceptButton);

    await waitFor(() => {
      expect(global.fetch).toHaveBeenCalledWith(
        expect.stringContaining("/api/improve/suggestions/sugg-1/accept"),
        expect.objectContaining({
          method: "POST",
        }),
      );
    });
  });
});
