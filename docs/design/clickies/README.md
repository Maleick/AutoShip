# Clicky Item Automation Design Docs

Design documents for the clicky (consumable/click-item) automation system.

| Doc | Summary |
|---|---|
| [api-design.md](api-design.md) | Public API surface for `ClickyItem` + `ClickyManager` |
| [architecture.md](architecture.md) | Module layout and component responsibilities |
| [config-examples.md](config-examples.md) | Example `config/clickies/*.toml` configurations |
| [rgmercs-review.md](rgmercs-review.md) | How this approach compares to rgmercs' clicky handling |

Implementation lives in [`textquest/src/camp/clickies.rs`](../../../textquest/src/camp/clickies.rs).
Config loader and TOML schemas: see [`#1040`](https://github.com/Maleick/TextQuest/pull/2327).
