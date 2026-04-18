import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { AdminSessionOverview } from "./AdminSessionOverview";

describe("AdminSessionOverview", () => {
  it("renders a loading state while the admin inventory is in flight", () => {
    render(
      <AdminSessionOverview
        sessions={[]}
        loading
        error={null}
      />
    );

    expect(screen.getByText(/loading admin session inventory/i)).toBeInTheDocument();
  });

  it("renders an explicit empty state when no managed sessions are returned", () => {
    render(
      <AdminSessionOverview
        sessions={[]}
        loading={false}
        error={null}
      />
    );

    expect(screen.getByText(/no managed sessions reported by the admin api/i)).toBeInTheDocument();
  });
});
