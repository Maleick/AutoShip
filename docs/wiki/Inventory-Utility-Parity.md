# Inventory Utility Parity

This page tracks the `TextQuest#1692` parity pack for RedGuides item-knowledge
and inventory-utility extensions. The shared config surface lives in the web
dashboard under **Loot Config > Inventory Utilities** and persists through
`GET` and `PUT /api/loot/inventory-utility`.

## Plugin Mapping

| Plugin | Status | Owner | Surface | Notes |
| --- | --- | --- | --- | --- |
| `MQ2LinkDB` | Adapted | `inventory_utility.item_knowledge` | Loot Config > Inventory Utilities | Uses TextQuest item-knowledge sources plus provenance instead of direct in-game link generation. |
| `MQ2ItemScore` | Native | `loot::item_score` | Loot Config > Item Score | Native weighted upgrade scoring drives keep or sell fallback decisions. |
| `MQ2Cursor` | Adapted | `inventory_utility.cursor_rules` | Loot Config > Inventory Utilities | Cursor retention and overflow handling are modeled as shared keep, sell, destroy, or consume rules. |
| `MQ2Collections` | Adapted | `inventory_utility.collection_routing` | Loot Config > Inventory Utilities | Collection set routing uses keep, bank, tribute, and sell actions. |
| `MQ2Collectible` | Native | `camp::collectibles` | Loot Config > Inventory Utilities | Existing collectible scanning and tribute automation continue to drive collection completion behavior. |
| `MQ2TributeManager` | Native | `camp::collectibles::TributeAutomationController` | Tuning Panel > Tribute Automation | Tribute expiry and activation are handled natively, with inventory utility docs tracking the parity status. |
| `MQ2TSTrophy` | Deferred | `inventory_utility.trophy_preferences` | Loot Config > Inventory Utilities | Operator preferences are tracked now; full trophy equip automation is explicitly deferred. |
| `MQ2Rewards` | Native | `inventory_utility.reward_routing` | Loot Config > Inventory Utilities | Task reward selection and claim intent route through normalized reward rules. |
| `MQ2FeedMe` | Adapted | `inventory_utility.consumption` | Loot Config > Inventory Utilities | Preferred food and drink plus ignored items replace plugin-local consumption rules. |
| `MQ2PortalSetter` | Adapted | `inventory_utility.relocation_rules` | Loot Config > Inventory Utilities | Portal presets are modeled as relocation destinations backed by clickies or AAs. |
| `MQ2Relocate` | Native | `nav::relocate` | Loot Config > Inventory Utilities | Travel routing already selects the best ready relocation option per destination. |
| `MQ2Vendors` | Adapted | `inventory_utility.vendor_watch` | Loot Config > Inventory Utilities | Vendor watch rules complement the existing sell-cycle controller with watched-item alerts. |
| `MQ2AutoClaim` | Adapted | `inventory_utility.auto_claim` | Loot Config > Inventory Utilities | Claim policies are tracked in the shared inventory surface while popup automation remains adapter-backed. |

## Provenance and Unsupported Fields

Adapted plugin settings expose provenance and unsupported-field warnings in the
dashboard so operators can see what was preserved and what still needs manual
review.

Current adapted warning sources:

- `MQ2LinkDB` from `legacy_ini:itemdb`
- `MQ2Cursor` from `legacy_ini:cursor`
- `MQ2PortalSetter` from `legacy_ini:portalsetter`

Current unsupported legacy fields called out in the UI:

- custom link color formatting
- chat-channel output templates
- drop-on-ground overflow actions
- campfire placement macros

## Deferred Scope

`MQ2TSTrophy` is currently marked `Deferred` in the default inventory utility
config. The shared UI already captures trophy preferences and provenance, but
full equip and restore automation remains explicitly deferred until the native
controller work lands.
