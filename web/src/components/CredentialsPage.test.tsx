import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import CredentialsPage from "./CredentialsPage";

// Mock the useAccounts hook
vi.mock("../hooks/useAccounts", () => ({
  useAccounts: () => ({
    accounts: [],
    loading: false,
    error: null,
    refresh: vi.fn(),
    createAccount: vi.fn(),
    updateAccount: vi.fn(),
    deleteAccount: vi.fn(),
    exportAccounts: vi.fn(),
    importAccounts: vi.fn(),
  }),
}));

describe("CredentialsPage", () => {
  it("renders the credentials management header", () => {
    render(<CredentialsPage />);
    expect(screen.getByText("Credentials Management")).toBeInTheDocument();
  });

  it("displays empty state when no credentials", () => {
    render(<CredentialsPage />);
    expect(screen.getByText(/No credentials stored yet/i)).toBeInTheDocument();
  });

  it("renders action buttons", () => {
    render(<CredentialsPage />);
    expect(screen.getByText(/Add Credential/i)).toBeInTheDocument();
    expect(screen.getByText(/Export/i)).toBeInTheDocument();
    expect(screen.getByText(/Import/i)).toBeInTheDocument();
  });
});
