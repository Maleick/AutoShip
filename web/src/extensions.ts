export type ExtensionDomain =
  | "combat"
  | "navigation"
  | "loot"
  | "awareness"
  | "economy"
  | "operator_utilities";

export type CompatibilityTier = "native" | "adapted" | "legacy";
export type ExtensionSourceKind = "textquest_native" | "legacy_profile";
export type ConfigProvenanceKind = "extension_catalog" | "legacy_import";
export type ScopeKind = "character" | "group" | "session";
export type FieldKind = "boolean" | "integer" | "string" | "enum" | "string_array";
export type AdapterHealth = "healthy" | "degraded" | "disabled";

export interface ExtensionFieldOption {
  value: string;
  label: string;
}

export interface ExtensionFieldSchema {
  key: string;
  label: string;
  description: string;
  kind: FieldKind;
  required: boolean;
  options?: ExtensionFieldOption[];
  min?: number;
  max?: number;
  defaultValue: unknown;
}

export interface ConfigProvenanceInfo {
  kind: ConfigProvenanceKind;
  label: string;
  path: string | null;
}

export interface ExtensionRuntimeStatus {
  enabled: boolean;
  adapterHealth: AdapterHealth;
  degradedReason?: string | null;
  lastSyncAt?: string | null;
  lastSyncMessage: string;
}

export interface ScopeRef {
  kind: ScopeKind;
  id: string;
}

export interface ExtensionScopeOverride {
  scope: ScopeRef;
  settings: Record<string, unknown>;
}

export interface ExtensionCatalogEntry {
  id: string;
  displayName: string;
  description: string;
  domain: ExtensionDomain;
  compatibilityTier: CompatibilityTier;
  sourceKind: ExtensionSourceKind;
  configProvenance: ConfigProvenanceInfo;
  supportedScopes: ScopeKind[];
  schema: ExtensionFieldSchema[];
  settings: Record<string, unknown>;
  overrides: ExtensionScopeOverride[];
  runtime: ExtensionRuntimeStatus;
  unsupportedFields: string[];
  legacySourceName?: string | null;
}

export interface ExtensionRuntimeEvent {
  type: "extension.runtime";
  entry: ExtensionCatalogEntry;
}
