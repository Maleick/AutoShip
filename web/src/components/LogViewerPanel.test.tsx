import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { LogViewerPanel } from "./LogViewerPanel";

vi.mock("../hooks/useAdminLogs", () => ({
  useAdminLogs: vi.fn(),
}));

import { useAdminLogs } from "../hooks/useAdminLogs";

describe("LogViewerPanel", () => {
  it("renders a loading state while logs are in flight", () => {
    vi.mocked(useAdminLogs).mockReturnValue({
      logs: [],
      loading: true,
      error: null,
    });

    render(<LogViewerPanel />);
    expect(screen.getByText(/loading/i)).toBeInTheDocument();
  });

  it("renders an error state when logs cannot be loaded", () => {
    vi.mocked(useAdminLogs).mockReturnValue({
      logs: [],
      loading: false,
      error: "Service unavailable",
    });

    render(<LogViewerPanel />);
    expect(screen.getByText(/service unavailable/i)).toBeInTheDocument();
  });

  it("renders an empty state when no logs are available", () => {
    vi.mocked(useAdminLogs).mockReturnValue({
      logs: [],
      loading: false,
      error: null,
    });

    render(<LogViewerPanel />);
    expect(screen.getByText(/no logs available/i)).toBeInTheDocument();
  });

  it("renders log entries when successfully loaded", () => {
    vi.mocked(useAdminLogs).mockReturnValue({
      logs: [
        {
          timestamp: new Date().toISOString(),
          level: "info",
          message: "System started",
          source: "main",
        },
        {
          timestamp: new Date().toISOString(),
          level: "error",
          message: "Connection failed",
          source: "network",
        },
      ],
      loading: false,
      error: null,
    });

    render(<LogViewerPanel />);
    expect(screen.getByText(/system started/i)).toBeInTheDocument();
    expect(screen.getByText(/connection failed/i)).toBeInTheDocument();
    expect(screen.getByText(/2 entries/i)).toBeInTheDocument();
  });

  it("styles error logs differently from info logs", () => {
    vi.mocked(useAdminLogs).mockReturnValue({
      logs: [
        {
          timestamp: new Date().toISOString(),
          level: "error",
          message: "Error log",
          source: "test",
        },
      ],
      loading: false,
      error: null,
    });

    render(<LogViewerPanel />);
    const logEntry = screen.getByText(/error log/i);
    expect(logEntry).toBeInTheDocument();
  });
});