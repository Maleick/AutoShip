import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import ExtensionCatalogPanel from "./ExtensionCatalogPanel";
import { useExtensionCatalog } from "../hooks/useExtensionCatalog";

vi.mock("../hooks/useExtensionCatalog", () => ({
  useExtensionCatalog: vi.fn(),
}));

const saveSettings = vi.fn();
const saveScopeOverride = vi.fn();
const deleteScopeOverride = vi.fn();
const saveRuntime = vi.fn();

describe("ExtensionCatalogPanel", () => {
  beforeEach(() => {
    saveSettings.mockReset();
    saveScopeOverride.mockReset();
    deleteScopeOverride.mockReset();
    saveRuntime.mockReset();

    vi.mocked(useExtensionCatalog).mockReturnValue({
      entries: [
        {
          id: "kissassist_profile",
          displayName: "KissAssist Imported Profile",
          description: "Imported legacy INI profile",
          domain: "combat",
          compatibilityTier: "legacy",
          sourceKind: "legacy_profile",
          configProvenance: {
            kind: "legacy_import",
            label: "Imported legacy profile",
            path: null,
          },
          supportedScopes: ["character", "group", "session"],
          schema: [
            {
              key: "enabled",
              label: "Enabled",
              description: "Enable the imported profile.",
              kind: "boolean",
              required: true,
              defaultValue: false,
            },
            {
              key: "assistName",
              label: "Assist Name",
              description: "Primary assist character.",
              kind: "string",
              required: true,
              defaultValue: "TankName",
            },
          ],
          settings: {
            enabled: false,
            assistName: "TankName",
          },
          overrides: [
            {
              scope: { kind: "character", id: "Frostreaver" },
              settings: {
                enabled: true,
                assistName: "Frostreaver",
              },
            },
          ],
          runtime: {
            enabled: true,
            adapterHealth: "degraded",
            degradedReason: "Unsupported imported settings require manual adapter review.",
            lastSyncAt: "2026-04-17T10:00:00Z",
            lastSyncMessage: "Enabled in degraded mode with 2 unsupported fields",
          },
          unsupportedFields: ["MeleeStickRange", "PetBurnAlways"],
          legacySourceName: "kissassist.ini",
        },
      ],
      loading: false,
      error: null,
      refresh: vi.fn().mockResolvedValue(undefined),
      saveSettings: saveSettings.mockResolvedValue(undefined),
      saveScopeOverride: saveScopeOverride.mockResolvedValue(undefined),
      deleteScopeOverride: deleteScopeOverride.mockResolvedValue(undefined),
      saveRuntime: saveRuntime.mockResolvedValue(undefined),
    });
  });

  it("renders degraded legacy metadata and supports save, revert, and override actions", async () => {
    render(<ExtensionCatalogPanel />);

    expect(
      screen.getByRole("heading", { name: /extension catalog/i })
    ).toBeInTheDocument();
    expect(screen.getAllByText(/legacy profile/i).length).toBeGreaterThan(0);
    expect(screen.getByText(/manual adapter review/i)).toBeInTheDocument();
    expect(screen.getByText(/MeleeStickRange/i)).toBeInTheDocument();

    const baseSettingsSection = screen
      .getByRole("heading", { name: /base settings/i })
      .closest("section");
    expect(baseSettingsSection).not.toBeNull();

    const assistInput = within(baseSettingsSection as HTMLElement).getAllByRole("textbox")[0];
    fireEvent.change(assistInput, { target: { value: "RaidTank" } });
    fireEvent.click(screen.getByRole("button", { name: /^save settings$/i }));

    await waitFor(() =>
      expect(saveSettings).toHaveBeenCalledWith("kissassist_profile", {
        enabled: false,
        assistName: "RaidTank",
      })
    );

    fireEvent.change(assistInput, { target: { value: "TemporaryTank" } });
    fireEvent.click(screen.getByRole("button", { name: /^revert settings$/i }));
    expect(
      within(baseSettingsSection as HTMLElement).getAllByRole("textbox")[0]
    ).toHaveValue("TankName");

    fireEvent.change(screen.getByLabelText(/scope id/i), {
      target: { value: "RaidLead" },
    });
    fireEvent.click(screen.getByRole("button", { name: /save override/i }));

    await waitFor(() =>
      expect(saveScopeOverride).toHaveBeenCalledWith(
        "kissassist_profile",
        "character",
        "RaidLead",
        {
          enabled: false,
          assistName: "TankName",
        }
      )
    );

    fireEvent.click(screen.getByRole("button", { name: /delete override/i }));
    expect(deleteScopeOverride).toHaveBeenCalledWith(
      "kissassist_profile",
      "character",
      "RaidLead"
    );

    fireEvent.click(screen.getByRole("button", { name: /disable runtime/i }));
    expect(saveRuntime).toHaveBeenCalledWith("kissassist_profile", false);
  });

  it("preserves unsaved base settings when only runtime metadata changes", async () => {
    const hookState = {
      entries: [
        {
          id: "kissassist_profile",
          displayName: "KissAssist Imported Profile",
          description: "Imported legacy INI profile",
          domain: "combat" as const,
          compatibilityTier: "legacy" as const,
          sourceKind: "legacy_profile" as const,
          configProvenance: {
            kind: "legacy_import" as const,
            label: "Imported legacy profile",
            path: null,
          },
          supportedScopes: ["character", "group", "session"] as const,
          schema: [
            {
              key: "enabled",
              label: "Enabled",
              description: "Enable the imported profile.",
              kind: "boolean" as const,
              required: true,
              defaultValue: false,
            },
            {
              key: "assistName",
              label: "Assist Name",
              description: "Primary assist character.",
              kind: "string" as const,
              required: true,
              defaultValue: "TankName",
            },
          ],
          settings: {
            enabled: false,
            assistName: "TankName",
          },
          overrides: [],
          runtime: {
            enabled: false,
            adapterHealth: "disabled" as const,
            lastSyncMessage: "Runtime disabled",
          },
          unsupportedFields: [],
          legacySourceName: null,
        },
      ],
      loading: false,
      error: null,
      refresh: vi.fn().mockResolvedValue(undefined),
      saveSettings: saveSettings.mockResolvedValue(undefined),
      saveScopeOverride: saveScopeOverride.mockResolvedValue(undefined),
      deleteScopeOverride: deleteScopeOverride.mockResolvedValue(undefined),
      saveRuntime: saveRuntime.mockResolvedValue(undefined),
    };
    vi.mocked(useExtensionCatalog).mockImplementation(() => hookState);

    const { rerender } = render(<ExtensionCatalogPanel />);

    const baseSettingsSection = screen
      .getByRole("heading", { name: /base settings/i })
      .closest("section");
    expect(baseSettingsSection).not.toBeNull();

    const assistInput = within(baseSettingsSection as HTMLElement).getAllByRole("textbox")[0];
    fireEvent.change(assistInput, { target: { value: "UnsavedTank" } });
    expect(assistInput).toHaveValue("UnsavedTank");

    hookState.entries = [
      {
        ...hookState.entries[0],
        runtime: {
          enabled: true,
          adapterHealth: "healthy",
          lastSyncAt: "2026-04-17T12:00:00Z",
          lastSyncMessage: "Runtime toggle applied from dashboard",
        },
      },
    ];

    rerender(<ExtensionCatalogPanel />);

    expect(within(baseSettingsSection as HTMLElement).getAllByRole("textbox")[0]).toHaveValue(
      "UnsavedTank"
    );
  });

  it("recovers selection when refreshed entries drop the selected extension id", async () => {
    const hookState = {
      entries: [
        {
          id: "kissassist_profile",
          displayName: "KissAssist Imported Profile",
          description: "Imported legacy INI profile",
          domain: "combat" as const,
          compatibilityTier: "legacy" as const,
          sourceKind: "legacy_profile" as const,
          configProvenance: {
            kind: "legacy_import" as const,
            label: "Imported legacy profile",
            path: null,
          },
          supportedScopes: ["character", "group", "session"] as const,
          schema: [
            {
              key: "enabled",
              label: "Enabled",
              description: "Enable the imported profile.",
              kind: "boolean" as const,
              required: true,
              defaultValue: false,
            },
          ],
          settings: {
            enabled: false,
          },
          overrides: [],
          runtime: {
            enabled: false,
            adapterHealth: "disabled" as const,
            lastSyncMessage: "Runtime disabled",
          },
          unsupportedFields: [],
          legacySourceName: null,
        },
        {
          id: "nav_mesh",
          displayName: "Navigation Mesh Controller",
          description: "Live navigation controls",
          domain: "navigation" as const,
          compatibilityTier: "native" as const,
          sourceKind: "textquest_native" as const,
          configProvenance: {
            kind: "native_defaults" as const,
            label: "TextQuest defaults",
            path: null,
          },
          supportedScopes: ["character", "group", "session"] as const,
          schema: [
            {
              key: "enabled",
              label: "Enabled",
              description: "Enable navigation controls.",
              kind: "boolean" as const,
              required: true,
              defaultValue: true,
            },
          ],
          settings: {
            enabled: true,
          },
          overrides: [],
          runtime: {
            enabled: true,
            adapterHealth: "healthy" as const,
            lastSyncAt: "2026-04-17T12:00:00Z",
            lastSyncMessage: "Runtime toggle applied from dashboard",
          },
          unsupportedFields: [],
          legacySourceName: null,
        },
      ],
      loading: false,
      error: null,
      refresh: vi.fn().mockResolvedValue(undefined),
      saveSettings: saveSettings.mockResolvedValue(undefined),
      saveScopeOverride: saveScopeOverride.mockResolvedValue(undefined),
      deleteScopeOverride: deleteScopeOverride.mockResolvedValue(undefined),
      saveRuntime: saveRuntime.mockResolvedValue(undefined),
    };
    vi.mocked(useExtensionCatalog).mockImplementation(() => hookState);

    const { rerender } = render(<ExtensionCatalogPanel />);

    await waitFor(() =>
      expect(
        screen.getByRole("heading", { name: /kissassist imported profile/i })
      ).toBeInTheDocument()
    );

    hookState.entries = [hookState.entries[1]];
    rerender(<ExtensionCatalogPanel />);

    await waitFor(() =>
      expect(
        screen.getByRole("heading", { name: /navigation mesh controller/i })
      ).toBeInTheDocument()
    );
    expect(screen.queryByText(/no extension definitions are available yet/i)).toBeNull();
  });
});
