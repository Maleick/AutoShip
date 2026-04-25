# Plugin Compatibility Matrix

This document tracks compatibility between TextQuest plugin API versions, plugin versions, and EQ implementations.

## API Version History

| TextQuest Version | Plugin API | Status | Notable Changes |
|-------------------|-----------|--------|-----------------|
| 0.1.0-alpha | 1.0 | Current | Initial plugin trait, PluginRegistry, manifest-based dependencies |

## Plugin Compatibility Chart

### Plugin API 1.0 (Current)

All plugins in this table target API version 1.0, which includes:

- `TextQuestPlugin` trait with `metadata()`, `on_load()`, `on_unload()`
- `PluginMetadata` with name, version, description, capabilities, manifest
- `PluginCapability` with domain, name, description
- `PluginManifest` with requires, force_unload, pause_on_load
- `PluginContext` for registering capabilities at load time
- `PluginRegistry` for lifecycle and conflict management

| Plugin Name | Version | Requires | Conflicts | Domains | Status | Notes |
|-------------|---------|----------|-----------|---------|--------|-------|
| example-inventory-plugin | 0.1.0 | None | None | Inventory | Reference | Example of simple capability registration |
| test-hello-plugin | 0.1.0 | None | None | None | Test | Minimal plugin for system verification |
| test-api-plugin | 0.1.0 | None | None | Combat, Navigation, Inventory | Test | Demonstrates multi-capability registration |
| test-error-plugin | 0.1.0 | None | None | None | Test | Demonstrates error handling and isolation |

## Domain Support Matrix

### Combat Domain

Plugins extending combat rotations, assist logic, and targeting:

| Plugin | Capability | Description | Status |
|--------|-----------|-------------|--------|
| test-api-plugin | combat.test-rotation | Test rotation capability | Test |

### Navigation Domain

Plugins extending pathfinding, camp movement, and zone handling:

| Plugin | Capability | Description | Status |
|--------|-----------|-------------|--------|
| test-api-plugin | navigation.test-pathfinding | Test pathfinding capability | Test |

### Inventory Domain

Plugins extending loot management, banking, and item sorting:

| Plugin | Capability | Description | Status |
|--------|-----------|-------------|--------|
| example-inventory-plugin | inventory.audit | Inventory audit extension point | Reference |
| test-api-plugin | inventory.test-loot | Test loot management capability | Test |

### System Domain

Runtime-level extensions not fitting other domains:

| Plugin | Capability | Description | Status |
|--------|-----------|-------------|--------|
| (none yet) | - | - | - |

## Feature Compatibility

### Dependency Resolution

Plugins can declare that other plugins must be loaded first:

```
test-plugin-a (requires: []) → Loads immediately if no conflicts
test-plugin-b (requires: [test-plugin-a]) → Loads only if test-plugin-a is loaded and enabled
```

**Matrix:**

| Plugin A | Plugin B | B requires A? | Can load B? | Notes |
|----------|----------|---------------|------------|-------|
| Not loaded | test | No | Yes | Independent |
| Loaded | test | No | Yes | Independent |
| Not loaded | test | Yes | No | Missing dependency |
| Loaded | test | Yes | Yes | Dependency satisfied |
| Disabled | test | Yes | No | Dependency disabled |

### Conflict Resolution

Plugins can declare incompatible plugins to be unloaded:

```
conflicting-plugin-a (force_unload: [some-plugin])
```

When conflicting-plugin-a loads, some-plugin is disabled automatically.

**Matrix:**

| Plugin A | Plugin B | A conflicts B? | B unloaded? | Notes |
|----------|----------|----------------|------------|-------|
| Loading | Not loaded | Yes | N/A | B not present |
| Loading | Disabled | Yes | N/A | B already disabled |
| Loading | Enabled | Yes | Yes | B unloaded by A |
| Loading | Enabled, can't unload | Yes | Error | A fails to load |

## Registry Behavior

### Lifecycle Enforcement

| Action | Precondition | Postcondition | Notes |
|--------|-------------|---------------|-------|
| Register | Metadata valid, name unique | Plugin enabled if all checks pass | Dependency/conflict checks enforce manifest |
| Enable | Plugin disabled, exists | Plugin enabled if on_load succeeds | Calls on_load again |
| Disable | Plugin enabled, exists | Plugin disabled if on_unload succeeds | Calls on_unload once |
| Unload (force) | Plugin exists | Plugin removed from registry | Used for cleanup; not exposed by default |

### Error Handling Behavior

| Error Type | Cause | Effect | Recovery |
|------------|-------|--------|----------|
| EmptyName | Plugin name is "" | Rejected at registration | Provide non-empty name |
| DuplicatePlugin | Name already registered | Rejected at registration | Use different name |
| NotFound | Plugin name not in registry | Operation fails | Plugin must be registered first |
| Lifecycle | on_load or on_unload fails | Plugin not registered (or remains disabled) | Fix error condition in plugin |
| UnsatisfiedRequirement | Required plugin absent | Rejected at registration | Load required plugin first |
| ConflictingPlugin | force_unload target can't be disabled | Rejected at registration | Resolve conflict manually or use different plugin |

### Capability Visibility

| Plugin Status | Capabilities Visible | Notes |
|---------------|-------------------|-------|
| Enabled | Yes | Capabilities returned by `capabilities(domain)` |
| Disabled | No | Capabilities hidden until plugin re-enabled |
| Failed on load | No | Plugin not in registry; no capabilities |
| Failed on unload | No | Plugin marked disabled; capabilities hidden |

## EQ Implementation Compatibility

TextQuest plugins are EQ-agnostic and compatible with:

- **Live servers** (all progression ruleset variants)
- **Test servers**
- **True Box servers** (with multi-box coordination)
- **Progression servers** (TLP, etc.)
- **Custom servers** (private servers with standard EQ protocol)

Plugins DO NOT require server-specific code. The TextQuest runtime handles protocol abstraction.

## Version Pinning

To ensure consistent behavior across upgrades:

1. **Plugins declare API version** (planned future feature):
   ```rust
   PluginMetadata::new(...)
       .with_api_version("1.0.0")
   ```

2. **Registry rejects incompatible plugins** (future):
   ```
   Plugin targets API 2.0, but registry provides 1.0 → Rejected
   ```

## Planned Compatibility Extensions

- [ ] Plugin versioning constraints (e.g., "requires plugin X >= 1.5.0")
- [ ] API versioning in registry (current: implicit API 1.0)
- [ ] Capability versioning (current: unversioned)
- [ ] Runtime capability negotiation

## Testing Compatibility

All plugins in this matrix pass:

- **Unit tests**: `cargo test -p <plugin>`
- **Integration tests**: `cargo test --test plugin_integration`
- **Registry isolation**: Failed plugins don't affect registry state
- **Lifecycle correctness**: on_load/on_unload called correctly

See `textquest-common/tests/plugin_integration.rs` for full test coverage.
