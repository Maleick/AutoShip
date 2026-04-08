import { renderHook, waitFor, act } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useAccounts } from "./useAccounts";
import { jsonResponse } from "../test/http";

describe("useAccounts", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("loads accounts, updates encoded routes, and parses API errors", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock
      .mockResolvedValueOnce(
        jsonResponse([
          {
            id: "1",
            name: "Bravo",
            server: "Teek",
            character: "Bardy",
            class: "BRD",
            group: 2,
            status: "active",
            has_password: false,
          },
        ])
      )
      .mockResolvedValueOnce(
        jsonResponse(
          {
            id: "2",
            name: "Alpha Prime",
            server: "Teek",
            character: "Alpha",
            class: "WAR",
            group: 1,
            status: "active",
            has_password: true,
          },
          { status: 201 }
        )
      )
      .mockResolvedValueOnce(
        jsonResponse({
          id: "2",
          name: "Alpha Prime",
          server: "Mischief",
          character: "Alpha Prime",
          class: "WAR",
          group: 1,
          status: "locked",
          has_password: true,
        })
      )
      .mockResolvedValueOnce(jsonResponse({ error: "cannot delete" }, { status: 400 }));

    const { result } = renderHook(() => useAccounts());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.accounts).toHaveLength(1);
    expect(result.current.accounts[0].name).toBe("Bravo");

    let created;
    await act(async () => {
      created = await result.current.createAccount({
        name: "Alpha",
        server: "Teek",
        character: "Alpha",
        class: "WAR",
        group: 1,
        status: "active",
      });
    });
    expect(created).toBeDefined();
    expect(created.name).toBe("Alpha Prime");

    let updated;
    await act(async () => {
      updated = await result.current.updateAccount("Alpha Prime", {
        server: "Mischief",
        character: "Alpha Prime",
        class: "WAR",
        group: 1,
        status: "locked",
      });
    });
    expect(updated).toBeDefined();
    expect(updated.server).toBe("Mischief");

    await expect(result.current.deleteAccount("Alpha Prime")).rejects.toThrow(
      "cannot delete"
    );

    expect(fetchMock).toHaveBeenNthCalledWith(2, "/api/accounts", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        name: "Alpha",
        server: "Teek",
        character: "Alpha",
        class: "WAR",
        group: 1,
        status: "active",
      }),
    });
    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      "/api/accounts/Alpha%20Prime",
      expect.objectContaining({ method: "PUT" })
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      4,
      "/api/accounts/Alpha%20Prime",
      expect.objectContaining({ method: "DELETE" })
    );
  });

  it("exports and imports accounts while revoking the blob URL", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock
      .mockResolvedValueOnce(jsonResponse([]))
      .mockResolvedValueOnce(jsonResponse({ exported: true }))
      .mockResolvedValueOnce(jsonResponse({ imported: 2 }))
      .mockResolvedValueOnce(jsonResponse([]));

    const createObjectUrl = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:textquest");
    const revokeObjectUrl = vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => undefined);
    const click = vi.fn();
    const originalCreateElement = document.createElement.bind(document);
    const createElement = vi.spyOn(document, "createElement").mockImplementation(
      ((tagName: string, options?: ElementCreationOptions) => {
        if (tagName === "a") {
          return {
            click,
            href: "",
            download: "",
          } as unknown as HTMLAnchorElement;
        }
        return originalCreateElement(tagName, options);
      }) as typeof document.createElement
    );

    const { result } = renderHook(() => useAccounts());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.exportAccounts();
    });

    expect(createObjectUrl).toHaveBeenCalledTimes(1);
    expect(click).toHaveBeenCalledTimes(1);
    expect(revokeObjectUrl).toHaveBeenCalledWith("blob:textquest");

    const importFile = new File(
      [JSON.stringify({ accounts: [{ name: "One" }, { name: "Two" }] })],
      "accounts.json",
      { type: "application/json" }
    );

    const imported = await act(async () => result.current.importAccounts(importFile));
    expect(imported).toBe(2);
    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      "/api/accounts/import",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ accounts: [{ name: "One" }, { name: "Two" }] }),
      })
    );

    createElement.mockRestore();
  });
});
