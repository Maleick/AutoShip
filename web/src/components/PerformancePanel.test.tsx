import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PerformancePanel } from "./PerformancePanel";

vi.mock("../hooks/useAdminMetrics", () => ({
  useAdminMetrics: vi.fn(),
}));

import { useAdminMetrics } from "../hooks/useAdminMetrics";

describe("PerformancePanel", () => {
  it("renders a loading state while metrics are in flight", () => {
    vi.mocked(useAdminMetrics).mockReturnValue({
      metrics: null,
      loading: true,
      error: null,
    });

    render(<PerformancePanel />);
    expect(screen.getByText(/loading/i)).toBeInTheDocument();
  });

  it("renders an error state when metrics cannot be loaded", () => {
    vi.mocked(useAdminMetrics).mockReturnValue({
      metrics: null,
      loading: false,
      error: "Failed to connect",
    });

    render(<PerformancePanel />);
    expect(screen.getByText(/failed to connect/i)).toBeInTheDocument();
  });

  it("renders performance metrics when successfully loaded", () => {
    vi.mocked(useAdminMetrics).mockReturnValue({
      metrics: {
        response_time_ms: 45,
        uptime_secs: 7200,
        memory_usage_mb: 256,
        timestamp: new Date().toISOString(),
      },
      loading: false,
      error: null,
    });

    render(<PerformancePanel />);
    expect(screen.getByText(/45ms/i)).toBeInTheDocument();
    expect(screen.getByText(/256mb/i)).toBeInTheDocument();
  });
});