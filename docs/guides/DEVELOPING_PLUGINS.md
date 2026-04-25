# Developing Plugins for TextQuest

This guide walks through the TextQuest plugin system architecture, API contracts, and best practices for building and testing external plugins.

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Plugin Lifecycle](#plugin-lifecycle)
- [API Reference](#api-reference)
- [Capability Domains](#capability-domains)
- [Dependency Management](#dependency-management)
- [Error Handling](#error-handling)
- [Testing](#testing)
- [Best Practices](#best-practices)

## Overview

The TextQuest plugin system provides a lightweight, trait-based extension mechanism for adding new combat rotations, navigation logic, inventory management, and system-level features. Plugins are compiled as Rust crates that implement the `TextQuestPlugin` trait.

Key design principles:

- **Trait-based**: Plugins declare capabilities via the `TextQuestPlugin` trait and metadata registration
- **Registry-driven**: A central `PluginRegistry` manages lifecycle and conflict resolution
- **Isolated**: Failed plugins cannot corrupt the registry or affect already-loaded plugins
- **Dependency-aware**: Plugins can declare requirements and conflicts via manifest declarations

## Quick Start

### Minimal Plugin

Create a new plugin crate with dependencies on `textquest-common`:

```toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"

[dependencies]
textquest-common = { path = "../../textquest-common" }
```

Implement the `TextQuestPlugin` trait:

```rust
use textquest_common::plugins::{PluginContext, PluginMetadata, PluginResult, TextQuestPlugin};

#[derive(Default)]
pub struct MyPlugin;

impl TextQuestPlugin for MyPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata::new(
            "my-plugin",
            "0.1.0",
            "Brief description of what this plugin does.",
        )
    }

    fn on_load(&mut self, _context: &mut PluginContext) -> PluginResult<()> {
        println!("Plugin loaded!");
        Ok(())
    }
}

pub fn plugin() -> Box<dyn TextQuestPlugin> {
    Box::new(MyPlugin)
}
```

### Registering Capabilities

Capabilities represent features or hooks your plugin provides. Register them in `on_load`:

```rust
use textquest_common::plugins::{PluginCapability, PluginDomain};

fn on_load(&mut self, context: &mut PluginContext) -> PluginResult<()> {
    context.register_capability(PluginCapability::new(
        PluginDomain::Combat,
        "combat.my-rotation",
        "Custom melee rotation for warriors.",
    ));
    Ok(())
}
```

## Plugin Lifecycle

### Load

1. **Validation**: Registry validates plugin metadata (non-empty name, no duplicates)
2. **Dependency Check**: All `requires` plugins must be loaded and enabled
3. **Conflict Resolution**: All `force_unload` plugins are disabled
4. **on_load Hook**: Plugin's `on_load` is called; capabilities are registered
5. **Status**: Plugin is marked enabled in the registry

If any step fails, the plugin is NOT registered and the error is returned.

### Unload

1. **on_unload Hook**: Plugin's `on_unload` is called for cleanup
2. **Capability Cleanup**: Plugin's capabilities are removed from the registry
3. **Status**: Plugin is marked disabled

If unload fails, the plugin remains in the registry with error status.

## API Reference

### PluginMetadata

Static information declared by your plugin at registration:

```rust
pub struct PluginMetadata {
    pub name: String,           // Unique identifier
    pub version: String,         // Semantic version
    pub description: String,     // What the plugin does
    pub capabilities: Vec<PluginCapability>,  // Features provided
    pub manifest: PluginManifest,              // Runtime contract
}
```

### TextQuestPlugin Trait

```rust
pub trait TextQuestPlugin: Send {
    fn metadata(&self) -> PluginMetadata;
    fn on_load(&mut self, context: &mut PluginContext) -> PluginResult<()> {
        Ok(())
    }
    fn on_unload(&mut self) -> PluginResult<()> {
        Ok(())
    }
}
```

### PluginContext

Passed to `on_load` for runtime capability registration:

```rust
pub struct PluginContext {
    // ...
}

impl PluginContext {
    pub fn register_capability(&mut self, capability: PluginCapability);
    pub fn capabilities(&self) -> &[PluginCapability];
}
```

### PluginManifest

Declare runtime contracts (dependencies and conflicts):

```rust
pub struct PluginManifest {
    pub requires: Vec<String>,        // Must be loaded first
    pub force_unload: Vec<String>,    // Unload these on load
    pub pause_on_load: Option<String>, // EQ command to issue after load
}
```

Example:

```rust
PluginMetadata::new("my-plugin", "1.0.0", "desc")
    .with_manifest(PluginManifest {
        requires: vec!["base-plugin".to_string()],
        force_unload: vec!["conflicting-plugin".to_string()],
        pause_on_load: Some("/enc pause on".to_string()),
        ..Default::default()
    })
```

### PluginError

Standard error types:

```rust
pub enum PluginError {
    EmptyName,                                  // Plugin name is empty
    DuplicatePlugin(String),                    // Already registered
    NotFound(String),                           // Plugin not in registry
    Lifecycle(String),                          // on_load/on_unload failed
    UnsatisfiedRequirement { plugin, missing }, // Missing dependencies
    ConflictingPlugin { plugin, conflicts },    // Conflicting plugins couldn't unload
}
```

## Capability Domains

Plugins can register capabilities in the following domains:

- **Combat**: Rotations, assist logic, targeting, class strategy
- **Navigation**: Pathing, camp movement, zone-line handling
- **Inventory**: Loot management, banking, vendor interactions
- **System**: Runtime extensions that don't fit other domains

Example:

```rust
context.register_capability(PluginCapability::new(
    PluginDomain::Combat,
    "combat.warrior-rotation",
    "Optimized warrior attack sequence",
));
```

## Dependency Management

### Declaring Requirements

Plugins can declare that other plugins must be loaded first:

```rust
PluginMetadata::new("my-plugin", "1.0.0", "...")
    .with_manifest(PluginManifest {
        requires: vec!["required-plugin".to_string()],
        ..Default::default()
    })
```

The registry will prevent loading unless all `requires` plugins are present and enabled.

### Declaring Conflicts

Plugins can declare incompatible plugins that should be unloaded:

```rust
PluginManifest {
    force_unload: vec!["conflicting-plugin".to_string()],
    ..Default::default()
}
```

When your plugin loads, conflicting plugins are disabled. If any cannot be disabled, registration fails.

## Error Handling

### Graceful Failure

If `on_load` returns an error, the plugin is NOT registered:

```rust
fn on_load(&mut self, context: &mut PluginContext) -> PluginResult<()> {
    if some_condition_fails {
        return Err(PluginError::Lifecycle(
            "Configuration error: ...".to_string()
        ));
    }
    context.register_capability(...);
    Ok(())
}
```

### Error Isolation

Errors in one plugin don't affect others:

- Failed plugins are rejected at registration time
- Already-registered plugins remain unaffected
- Registry state is consistent before and after failure

## Testing

### Unit Tests

Test your plugin in isolation:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn my_plugin_loads() {
        let mut plugin = MyPlugin;
        let mut context = PluginContext::default();
        
        assert!(plugin.on_load(&mut context).is_ok());
    }

    #[test]
    fn my_plugin_registers_capabilities() {
        let mut plugin = MyPlugin;
        let mut context = PluginContext::default();
        
        plugin.on_load(&mut context).unwrap();
        
        assert_eq!(context.capabilities().len(), 1);
    }
}
```

### Integration Tests

Test plugin interaction with the registry. See `textquest-common/tests/plugin_integration.rs` for examples:

```rust
use textquest_common::plugins::PluginRegistry;

#[test]
fn plugin_loads_in_registry() {
    let mut registry = PluginRegistry::new();
    let plugin = MyPlugin;
    
    let result = registry.register(Box::new(plugin));
    assert!(result.is_ok());
    assert!(registry.status("my-plugin").is_some());
}
```

## Best Practices

### 1. Keep Plugin State Minimal

Plugins should be stateless when possible. If you need state, use `Send + Sync` types:

```rust
pub struct MyPlugin {
    enabled: bool,
    counters: Arc<Mutex<HashMap<String, usize>>>,
}
```

### 2. Use Descriptive Names

Capability names follow a `domain.feature` pattern:

```rust
// Good
"combat.warrior-rotation"
"navigation.camp-kiting"
"inventory.loot-sort"

// Avoid
"rotation"
"navigation"
"sort"
```

### 3. Document Your Manifest

If your plugin declares requirements or conflicts, document why:

```rust
/// Requires base-commands plugin for EQ command routing.
/// Forces unload of incompatible-rotation to prevent duplication.
pub fn metadata(&self) -> PluginMetadata {
    PluginMetadata::new(...)
        .with_manifest(PluginManifest {
            requires: vec!["base-commands".to_string()],
            force_unload: vec!["incompatible-rotation".to_string()],
            ..Default::default()
        })
}
```

### 4. Fail Early in on_load

Validate configuration and dependencies before registering capabilities:

```rust
fn on_load(&mut self, context: &mut PluginContext) -> PluginResult<()> {
    // Validate first
    self.validate_config()?;
    
    // Register capabilities
    context.register_capability(...);
    
    // Then initialize state
    self.initialize_state()?;
    
    Ok(())
}
```

### 5. Clean Up in on_unload

Release any resources (files, sockets, locks) when unloading:

```rust
fn on_unload(&mut self) -> PluginResult<()> {
    // Release resources
    self.cleanup_state()?;
    
    eprintln!("[{}] unloaded", self.metadata().name);
    Ok(())
}
```

### 6. Use Logging

Help operators debug plugin issues:

```rust
fn on_load(&mut self, context: &mut PluginContext) -> PluginResult<()> {
    eprintln!("[my-plugin] loading...");
    
    // ... registration ...
    
    eprintln!("[my-plugin] registered {} capabilities", context.capabilities().len());
    Ok(())
}
```

## Testing Your Plugin

1. **Add to workspace**: Add your plugin to `Cargo.toml` members list
2. **Write tests**: Add tests in `src/lib.rs` and `tests/` directory
3. **Run tests**: `cargo test -p my-plugin`
4. **Integration test**: Create a test in `textquest-common/tests/plugin_integration.rs`

## Compatibility

For version tracking, follow Semantic Versioning:

- **MAJOR**: Breaking API changes to `TextQuestPlugin` trait (rare)
- **MINOR**: New optional capabilities or features
- **PATCH**: Bug fixes and internal improvements

The `PluginMetadata.version` is informational and not enforced by the registry.
