import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { jsonResponse } from "../test/http";
import { useAdminSessions } from "./useAdminSessions";

describe("useAdminSessions", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn());
  });

  it("loads the admin session inventory from the dedicated endpoint", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockResolvedValueOnce(
      jsonResponse([
        {
          sessionId: 42,
          characterName: "Frostreaver",
          profile: "Cleric Anchor",
          groupId: "grp-1",
          routingScope: "Group grp-1",
          lifecycle: "live",
          status: "active",
          zone: "Plane of Fire",
          level: 60,
        },
      ])
    );

    const { result } = renderHook(() => useAdminSessions());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(fetchMock).toHaveBeenCalledWith("/api/admin/sessions");
    expect(result.current.error).toBeNull();
    expect(result.current.sessions).toEqual([
      expect.objectContaining({
        sessionId: "42",
        characterName: "Frostreaver",
        profile: "Cleric Anchor",
        groupId: "grp-1",
        routingScope: "Group grp-1",
        lifecycle: "live",
        status: "active",
        zone: "Plane of Fire",
        level: 60,
      }),
    ]);
  });
});
