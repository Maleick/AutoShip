# Vendor Cycle: Loot Routing & Overnight Farming

## Overview

The **Vendor Cycle** system automates the selling of unwanted loot to merchants, converting vendorable items into plat. It is the primary mechanism for converting overnight farming runs into currency, powering the economy layer alongside tradeskill trophy farming.

This page documents:
- How vendor cycling works operationally
- Configuration options (keep-lists, allowlists, backlog thresholds)
- Integration with overnight farming and the orchestration loop
- Economic assumptions and plat projections

---

## How It Works

### Operational Flow

1. **Loot Acquisition** → Combat or farming activities produce items that land in character inventory
2. **Inventory Snapshot** → The orchestrator captures current inventory state
3. **Vendor Planning** → The `VendorCyclePlanner` evaluates each item against configured rules
4. **Sale Execution** → Items marked for sale are vendored to an NPC merchant
5. **Plat Conversion** → Inventory is converted to currency, logged, and tracked

### Item Classification

The planner classifies each item according to **why** it should be sold:

| Reason | Criteria | Use Case |
|--------|----------|----------|
| **Trash** | Explicitly marked junk/vendor trash items | Vendor trash that has no other use; auto-detect via item naming patterns |
| **Duplicate Loot** | Extra copies of the same item in inventory | Keep one copy, vendor all duplicates (by acquisition time) |
| **Backlog** | Loot older than the backlog threshold | Keep items for N days (default 30); vendor anything beyond the age limit |
| **Allowlist** | Operator-configured explicit sell list | Selective vending when an allowlist is defined; only these items sell |
| **Manual** | Legacy queue entry (future use) | For operator-initiated vendoring outside the automatic planner |

### Planner State

The `VendorCyclePlanner` holds three configuration components:

1. **Keep-List** (`keep_names`) — Items to NEVER vendor (protected gear, quest items, crafting material, etc.)
2. **Allowlist** (`allowlist`) — When active, ONLY items on this list will vendor (overrides trash/backlog/duplicate logic)
3. **Backlog Threshold** (`backlog_days`) — Age in days beyond which old loot is vendored (default 30 days)

---

## Configuration Reference

### Creating a Planner

```rust
use textquest::loot::vendor_cycle::{VendorCyclePlanner, VendorInventoryItem};

// Basic planner with a keep-list
let planner = VendorCyclePlanner::new(vec!["Silk Silk", "Spells", "Trophy"]);

// With all options
let planner = VendorCyclePlanner::new(vec!["Silk Silk", "Spells", "Trophy"])
    .with_allowlist(vec!["Junk", "Vendor Trash", "Worn Bone"])
    .with_backlog_days(14);
```

### Keep-List (Protected Items)

The keep-list protects items from being vendored. Use this for:
- **Quest items** (flags, components, currency)
- **Crafting materials** (silk, bone, gems)
- **Gear you're using** (armor, weapons, accessories)
- **Future quest needs** (trophies, currency items)

**Name Matching:** Keep-list matches are **case-insensitive** and **whitespace-trimmed**. For example:
- `"Silk Silk"` will protect "Silk Silk", " Silk Silk ", "SILK SILK", etc.
- The exact item name does not need to be specified; the planner will match any item containing the protected name

### Allowlist (Explicit Sell List)

When an allowlist is defined, the planner enters **allowlist mode**:
- Only items whose names match entries in the allowlist will vendor
- Trash items, backlog, and duplicates are ignored unless they're also on the allowlist
- Use this for strict control: "vendor ONLY these items, nothing else"

**Example:** Overnight farming in Old Sebilis might configure:
```rust
.with_allowlist(vec![
    "Silkfang Mane",
    "Jug of Saltwater",
    "Myconid Chitin",
    "Ancient Silk",
])
```
Only these four item types will vendor. Everything else stays in inventory.

### Backlog Threshold

The backlog threshold is the **age in days** beyond which old loot gets vendored automatically.

- **Default:** 30 days
- **Use cases:**
  - `backlog_days(7)` — Aggressive cleanup; vendor anything over a week old
  - `backlog_days(60)` — Conservative; keep loot for 2 months
  - `backlog_days(0)` — No backlog vendoring; only trash/duplicates/allowlist sells

**Example:** Quest trophy farming might use a high backlog threshold to keep trophies on hand longer:
```rust
.with_backlog_days(90)  // Keep loot for 90 days before auto-vending
```

---

## Integration with Overnight Farming

### Farming Loop Integration

The vendor cycle sits within the broader overnight farming orchestration:

```
1. Combat Loop (6-12 hours)
   ↓ [loot drops to inventory]
2. Inventory Snapshot
   ↓ [captures item state]
3. Vendor Planning
   ↓ [evaluates each item]
4. Vendor Execution
   ↓ [sells marked items]
5. Plat Logging
   ↓ [records earned currency]
6. Repeat (next session)
```

### Economic Input

Each overnight farming run produces:
- **XP gains** (leveling progress)
- **Loot hauls** (items in inventory)
- **Tradeskill trophies** (if enabled)
- **Plat from vendors** (gross vendorable value)

The planner estimates gross plat value for each cycle but does NOT execute sales—that responsibility remains with the orchestrator or TUI operator.

### Plat Projection

Given a VendorPlan, you can estimate nightly earnings:

```rust
let plan = planner.plan(&inventory, Utc::now());
let gross_plat = plan.estimated_gross_plat;

// Example: nightly farm in Old Sebilis
// plan.estimated_gross_plat = 800pp per cycle
// 6 farming cycles per overnight run = 4,800pp
// 30 nights in a month = 144,000pp
```

### Interaction with Tradeskill Trophies

Tradeskill trophies (alchemy, jewelry, etc.) should typically be on the keep-list so they accumulate during overnight runs. The vendor cycle will not touch them.

For trophy *disposal* (after you've farmed the target amount), you would:
1. Move trophies off the keep-list manually
2. Or create a new allowlist that includes trophy names
3. Re-run vendor planning to liquidate

---

## Scenario Walkthrough

### Scenario 1: Generic Overnight Farm (Sebilis)

**Setup:**
```rust
let planner = VendorCyclePlanner::new(vec![
    "Nodding Blue Lily",       // Quest item, keep
    "Ancient Silkfang Mane",   // Trophy material, keep
    "Tarnished Plate Armor",   // Unsorted gear, keep for now
])
.with_backlog_days(30)
.with_allowlist(vec![]);  // No allowlist; use trash/backlog/duplicate logic
```

**Inventory snapshot contains:**
- 3x Silkfang Mane (acquired 5 days ago)
- 2x Silkfang Mane (acquired 12 days ago) — *duplicate*
- 1x Jug of Saltwater (acquired 2 days ago) — *trash item*
- 1x Nodding Blue Lily (acquired 3 days ago) — *protected*
- 1x Worn Bone Armor (acquired 45 days ago) — *backlog item*

**Planner decision:**
- 3x Silkfang Mane (5 days) → PROTECTED (on keep-list)
- 2x Silkfang Mane (12 days) → VENDOR (duplicate; keep newest)
- 1x Jug of Saltwater → VENDOR (trash)
- 1x Nodding Blue Lily → PROTECTED (on keep-list)
- 1x Worn Bone Armor → VENDOR (over 30-day backlog)

**Estimated vendor revenue:** (2×20pp) + (1×8pp) + (1×15pp) = **63pp gross**

---

### Scenario 2: Strict Allowlist (Trophy Farming)

**Setup:**
```rust
let planner = VendorCyclePlanner::new(vec![
    "Trophy of the Warlord",    // The trophy itself; keep until we hit target
    "Silk Silk",                // Crafting material
])
.with_allowlist(vec![
    "Worn Bone",
    "Chipped Weapon",
    "Tattered Hide",
    "Junk Jewel",
]);
```

**Inventory snapshot:**
- 1x Trophy of the Warlord (protected, never sells)
- 5x Silk Silk (protected)
- 3x Worn Bone (on allowlist) → VENDOR
- 2x Chipped Weapon (on allowlist) → VENDOR
- 1x Fancy Silk Robe (NOT on allowlist, NOT protected) → IGNORE
- 1x Ancient Spellbook (NOT on allowlist, NOT protected) → IGNORE

**Planner decision:**
Only the allowlist items vendor. Everything else stays in inventory for sorting or future use.

**Estimated vendor revenue:** (3×5pp) + (2×3pp) = **21pp gross**

---

## Implementation Details

### Item Name Matching

The planner normalizes all item names to **lowercase** and **trims whitespace**. This ensures:
- `"Silk Silk"` matches `" SILK SILK "`, `"Silk Silk"`, `"silk silk"`, etc.
- Keep-lists and allowlists are case-insensitive

### Duplicate Detection

For duplicate items, the planner:
1. Groups items by normalized name
2. Sorts each group by acquisition time (newest first)
3. Marks all items EXCEPT the newest as duplicates for vendoring

This ensures you always keep the **most recently acquired** copy and vendor older copies.

### Backlog Age Calculation

The backlog cutoff is calculated as:
```
backlog_cutoff = current_time - Duration::days(backlog_days)
```

Items with `acquired_at <= backlog_cutoff` are eligible for backlog vendoring.

### Estimated Vendor Value

Each item has an `estimated_vendor_value` field (in plat). The planner sums these for all sale items to produce `estimated_gross_plat`. This is a **projection**—actual merchant prices vary by NPC.

---

## Integration Points

### Orchestrator Loop

The orchestrator should:
1. Capture inventory state via IPC
2. Build a `VendorCyclePlanner` from configuration
3. Call `planner.plan(inventory_snapshot, now)` to get a `VendorPlan`
4. Log or display the plan to the operator
5. Execute the plan (vendor sale) based on operator approval or automation rules

### IPC Container Slots

Inventory comes from the DLL as `ContainerSlotInfo` structs. These can be converted to `VendorInventoryItem`:

```rust
let item = VendorInventoryItem::try_from(&container_slot)?;
let plan = planner.plan(&[item], Utc::now());
```

### Loot History Integration

If you're tracking loot acquisition timestamps in a database, use `VendorInventoryItem::from_loot_history`:

```rust
let row = loot_history.fetch_row(item_name)?;
let item = VendorInventoryItem::from_loot_history(&row, estimated_price);
let plan = planner.plan(&[item], Utc::now());
```

---

## Known Limitations & Future Work

### Current Gaps

1. **No merchant selection** — The planner does not evaluate NPC merchant types (poison, drink, spell). You must select the right NPC manually.
2. **No auction house integration** — Currently assumes vendor NPC sales only; bazaar posting would require separate logic.
3. **No price lookup** — `estimated_vendor_value` is static; no live price feed from game data.
4. **No bag optimization** — Planner doesn't consider bag space; assumes inventory reorganization is handled separately.

### Future Enhancements

- **Per-NPC routing** — Different vendors (potions, spells, armor) routed to appropriate NPCs
- **Bazaar posting** — Items above a plat threshold posted to bazaar instead of vendored
- **Dynamic pricing** — Integration with market data feeds for real-time value estimation
- **Inventory optimization** — Suggest bag/container rearrangement before vendoring
- **Scarcity tracking** — Items on a "rarity watch list" are held longer before vendoring

---

## Testing

The vendor cycle planner is tested in `textquest/src/loot/vendor_cycle.rs` with comprehensive unit tests:

```bash
cargo test --lib loot::vendor_cycle
```

Key test scenarios:
- Trash item vendoring
- Duplicate detection and aging
- Keep-list protection
- Allowlist mode
- Backlog threshold calculation
- Edge cases (empty inventory, zero backlog_days, all items protected, etc.)

---

## References

- **Source:** `textquest/src/loot/vendor_cycle.rs`
- **IPC Integration:** `textquest-common/src/ipc.rs` (ContainerSlotInfo)
- **Orchestrator:** `textquest/src/orchestrator.rs`
- **Related:** [Combat and Camp Loop](Combat-and-Camp-Loop.md), [Overnight Farming](Frostreaver-Farming-Guide.md)
