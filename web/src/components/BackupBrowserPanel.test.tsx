import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { BackupBrowserPanel } from "./BackupBrowserPanel";

vi.mock("../hooks/useAdminBackups", () => ({
  useAdminBackups: vi.fn(),
}));

import { useAdminBackups } from "../hooks/useAdminBackups";

describe("BackupBrowserPanel", () => {
  it("renders a loading state while backups are in flight", () => {
    vi.mocked(useAdminBackups).mockReturnValue({
      backups: [],
      loading: true,
      error: null,
    });

    render(<BackupBrowserPanel />);
    expect(screen.getByText(/loading backups/i)).toBeInTheDocument();
  });

  it("renders an error state when backups cannot be loaded", () => {
    vi.mocked(useAdminBackups).mockReturnValue({
      backups: [],
      loading: false,
      error: "Storage error",
    });

    render(<BackupBrowserPanel />);
    expect(screen.getByText(/storage error/i)).toBeInTheDocument();
  });

  it("renders an empty state when no backups are available", () => {
    vi.mocked(useAdminBackups).mockReturnValue({
      backups: [],
      loading: false,
      error: null,
    });

    render(<BackupBrowserPanel />);
    expect(screen.getByText(/no backups found/i)).toBeInTheDocument();
  });

  it("renders backup entries when successfully loaded", () => {
    vi.mocked(useAdminBackups).mockReturnValue({
      backups: [
        {
          id: "backup-1",
          created_at: "2024-01-15T10:00:00Z",
          size_bytes: 1024000,
          status: "completed",
          description: "Initial snapshot",
        },
        {
          id: "backup-2",
          created_at: "2024-01-16T10:00:00Z",
          size_bytes: 2048000,
          status: "pending",
        },
      ],
      loading: false,
      error: null,
    });

    render(<BackupBrowserPanel />);
    expect(screen.getByText(/2 available/i)).toBeInTheDocument();
    expect(screen.getByText(/initial snapshot/i)).toBeInTheDocument();
  });

  it("shows status badges for different backup states", () => {
    vi.mocked(useAdminBackups).mockReturnValue({
      backups: [
        {
          id: "backup-1",
          created_at: "2024-01-15T10:00:00Z",
          size_bytes: 1024000,
          status: "completed",
        },
        {
          id: "backup-2",
          created_at: "2024-01-16T10:00:00Z",
          size_bytes: 0,
          status: "pending",
        },
        {
          id: "backup-3",
          created_at: "2024-01-17T10:00:00Z",
          size_bytes: 0,
          status: "failed",
        },
      ],
      loading: false,
      error: null,
    });

    render(<BackupBrowserPanel />);
    expect(screen.getByText(/completed/i)).toBeInTheDocument();
    expect(screen.getByText(/pending/i)).toBeInTheDocument();
    expect(screen.getByText(/failed/i)).toBeInTheDocument();
  });

  it("renders restore and delete buttons for completed backups", () => {
    vi.mocked(useAdminBackups).mockReturnValue({
      backups: [
        {
          id: "backup-1",
          created_at: "2024-01-15T10:00:00Z",
          size_bytes: 1024000,
          status: "completed",
        },
      ],
      loading: false,
      error: null,
    });

    render(<BackupBrowserPanel />);
    expect(screen.getByTitle(/restore this backup/i)).toBeInTheDocument();
    expect(screen.getByTitle(/delete this backup/i)).toBeInTheDocument();
  });
});