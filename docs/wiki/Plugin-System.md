# Plugin System

TextQuest plugin support starts with a shared Rust API in `textquest-common`.
This is a partial OpenVanilla-parity scaffold: it defines the extension
contract, registry lifecycle, domain capability metadata, a scaffold tool, and
one non-core example plugin. Runtime dynamic library discovery is intentionally
left as follow-up work until the ABI boundary is finalized.

## Architecture

The plugin surface is centered on `textquest_common::plugins`:

- `TextQuestPlugin` is the standard plugin trait.
- `PluginMetadata` identifies a plugin and its advertised capabilities.
- `PluginCapability` describes one extension point in a domain.
- `PluginDomain` currently covers `Combat`, `Navigation`, `Inventory`, and
  `System`.
- `PluginRegistry` owns plugin instances, calls lifecycle hooks, tracks
  enabled state, and indexes capabilities by domain.
- `PluginFactory` is the common factory signature used by static registration
  today and future dynamic loaders.

The current registry is in-process. Runtimes can register factories from linked
crates, then query enabled capabilities without depending on plugin internals.

## Lifecycle

Plugins implement `metadata()` and may override lifecycle hooks:

```rust
use textquest_common::plugins::{
    PluginCapability, PluginContext, PluginDomain, PluginMetadata, PluginResult, TextQuestPlugin,
};

pub struct MyPlugin;

impl TextQuestPlugin for MyPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata::new("my-plugin", env!("CARGO_PKG_VERSION"), "Example plugin")
    }

    fn on_load(&mut self, context: &mut PluginContext) -> PluginResult<()> {
        context.register_capability(PluginCapability::new(
            PluginDomain::Inventory,
            "inventory.example",
            "Registers an inventory extension point.",
        ));
        Ok(())
    }
}
```

`PluginRegistry::register_factory()` creates a plugin, validates metadata, runs
`on_load()`, and marks the plugin enabled. `disable()` calls `on_unload()` and
hides its capabilities from domain queries. `enable()` runs `on_load()` again
and restores capability visibility.

## Example Plugin

`plugins/example-inventory-plugin` is a non-core crate that depends on
`textquest-common` and exports `plugin() -> Box<dyn TextQuestPlugin>`. It
registers an `Inventory` capability named `inventory.audit`, demonstrating that
new extension behavior can live outside the core crates.

## Scaffolding

Create a new plugin crate template with:

```bash
python3 scripts/mkplugin.py my-inventory-plugin
```

The tool creates `plugins/my-inventory-plugin` with a minimal `Cargo.toml`,
`src/lib.rs`, `TextQuestPlugin` implementation, lifecycle hook, and factory
function. Add the generated crate to `workspace.members` when it should compile
as part of the repository.

## Follow-Up Work

- Add dynamic plugin discovery and dylib loading once the FFI/ABI contract is
  stable.
- Wire registry initialization into the orchestrator and DLL runtime surfaces.
- Expand domain APIs beyond capability registration into concrete combat,
  navigation, and inventory hook contracts.
- Add operator configuration for enabling and disabling plugin modules.
