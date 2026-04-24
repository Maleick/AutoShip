# TextQuest Web UI — Config Scope

Distilled backend contract for the five daily-use operator config pages. All paths are prefixed `/api/` unless noted. Field names match Rust struct serialization (serde default, snake_case for enums).

Dashboard (monitoring) is covered by `Dashboard.html` reference. This doc covers the _configuration_ surface that makes the web UI useful.

---

## 1. Sessions — client lifecycle + command routing

**Purpose.** Operator starts, pauses, resumes, and re-groups individual EQ client sessions; relays chat commands to one, the group, or all.
**Frequency.** Daily — every operator session starts here.
**UX.** Dense grid/table with per-row action buttons. Multi-select for bulk ops. Command input with scope selector.
**Source.** `textquest-web/src/api/session_control.rs`

### Routes

```
GET  /sessions/control              → list all session control records
GET  /sessions/{id}/control         → single record
PUT  /sessions/{id}/pause           → pause session
PUT  /sessions/{id}/resume          → resume session
PUT  /sessions/{id}/group           → move to group (body: SetGroupRequest)
PUT  /sessions/{id}/broadcast-all   → toggle broadcast-all flag
POST /sessions/{id}/command         → relay /slashcommand (body: SlashCommandRequest)
```

### Shapes

```ts
SessionState      = "idle" | "active" | "paused" | "error"
RoutingScope      = "self" | "group" | "broadcast"
CommandScope      = "self" | "group" | "all"

SessionControlRecord {
  session_id: u32
  group_id: u8                // 1..8
  routing_scope: RoutingScope
  state: SessionState
}

SetGroupRequest { group_id: u8 }
SlashCommandRequest { command: string, scope?: CommandScope }
SlashCommandResponse { session_id, command, scope_used, accepted, message }
```

### UI shape

- Table rows: `session_id · character · class · group · scope · state · [Pause|Resume|Group ▾|Broadcast]`
- Bulk bar: "Selected: 5 · [Pause all] [Resume all] [Move to group ▾]"
- Command strip (bottom, sticky): `[scope ▾] [ / text input         ] [Send]` — mono font

---

## 2. Auto-Group — group composition + invite automation

**Purpose.** Operator defines who belongs in each group and their role; automation sends invites, waits for joins, runs a completion command.
**Frequency.** Weekly per roster change; daily when swapping compositions.
**UX.** Member list editor (add/remove/role pick) plus automation timing knobs. Start/reset buttons with live phase readout.
**Source.** `textquest-web/src/api/auto_group.rs`

### Routes

```
GET  /auto-group/         → AutoGroupStatus
PUT  /auto-group/         → PutAutoGroupConfig
POST /auto-group/start    → begin invite sequence
POST /auto-group/reset    → cancel/reset
```

### Shapes

```ts
GroupRole = "tank" | "healer" | "dps" | "support" | "puller" | "mez" | "slow"
            // confirm full enum from auto_group.rs::GroupRole

AutoGroupMember {
  name: string
  role: GroupRole
}

AutoGroupConfig {
  enabled: boolean
  members: AutoGroupMember[]
  completion_command?: string   // e.g. "/g ready"
  max_retries: u8
  invite_interval_ticks: u64    // EQ server ticks (~6s each)
  member_wait_ticks: u64
}

AutoGroupStatus {
  phase: string                 // "idle" | "inviting" | "waiting" | "done" | ...
  config: AutoGroupConfig
}
```

### UI shape

- Left: member list — drag-reorderable rows, name + `role ▾` per row, `[+ Add member]`
- Right: timing panel — three numeric steppers (max_retries, invite_interval_ticks, member_wait_ticks) with "ticks → seconds" helper labels; completion command text input; enabled toggle
- Footer: phase pill (live) + `[Start]` `[Reset]`

---

## 3. Loot — rules, filters, master looter, distribution

**Purpose.** Operator controls keep/sell/destroy lists, per-character filter overrides, master looter assignment, distribution method per item type, and reviews loot history.
**Frequency.** Daily tweaking during camp sessions; rules set-once and refined.
**UX.** Tabbed page: Rules · Filters · Master Looter · Distribution · History. Heavy on lists and per-item rule rows.
**Source.** `textquest-web/src/api/loot.rs`, mounted under `/api/loot/`.

### Routes

```
GET  /loot/rules                    PUT /loot/rules
GET  /loot/filters                  PUT /loot/filters/{character}
GET  /loot/master-looter            PUT /loot/master-looter
GET  /loot/distribution             PUT /loot/distribution
GET  /loot/history                  (query: ?search=&character=&limit=)
GET  /loot/item-score               PUT /loot/item-score
GET  /loot/inventory-utility        PUT /loot/inventory-utility
```

### Shapes

```ts
FilterAction       = "keep" | "sell" | "destroy" | "ignore"   // confirm enum
DistributionMethod = "round_robin" | "master_looter" | "random" | "need_before_greed"
                     // confirm enum from loot.rs::DistributionMethod

LootRulesPayload {
  keep_items: string[]
  sell_items: string[]
  destroy_items: string[]
  loot_all: boolean
  auto_split: boolean
}

ItemFilterEntry      { item_name: string, action: FilterAction }
CharacterLootFilter  { character: string, filters: ItemFilterEntry[] }

DistributionRule {
  id: string
  item_type: string              // e.g. "weapon" | "armor" | "spell"
  quality?: string               // e.g. "legendary" | "rare"
  method: DistributionMethod
}
DistributionConfig  { rules: DistributionRule[] }

MasterLooterPayload { character?: string }

LootHistoryEntry {
  id: u64
  timestamp: string
  item_name: string
  recipient: string
  source_mob?: string
  zone?: string
  quantity: u32
  assigned_by?: string
}
```

### UI shape

- Tabs: `Rules | Filters | Master | Distribution | History | Scoring`
- Rules tab: three columns (Keep / Sell / Destroy), each a chip list with search + paste-many textarea
- Filters tab: character selector → filter table (item name, action dropdown, remove button)
- Distribution tab: rule rows — item_type select + quality select + method select + remove
- History tab: searchable data table, mono font, color-coded recipient column

---

## 4. Economy — vendor routes, wealth, queues

**Purpose.** Operator defines vendor-sell routes (which NPCs in which zones take which item categories), sees plat/hour + vendor sales ledger, and watches the loot/vendor queue depth.
**Frequency.** Weekly for route setup; status glanced at daily.
**UX.** Top status strip (paused/active + cycles) + two panels: vendor route CRUD + ledger/queue readouts.
**Source.** `textquest-web/src/api/economy.rs` + VendorRoute CRUD in `api.rs`.

### Routes

```
GET  /economy/status                (status readout)
GET  /economy/settings              PUT /economy/settings
GET  /economy/wealth                → EconomyLedgerResponse
GET  /economy/queues                → EconomyQueuesResponse
GET  /economy/vendor-routes         POST /economy/vendor-routes
PUT  /economy/vendor-routes/{id}    DELETE /economy/vendor-routes/{id}
```

### Shapes

```ts
EconomyStatusResponse { active_cycles: string[], is_paused: boolean }
EconomyLedgerResponse {
  plat_per_hour: f64
  items_distributed: u64
  vendor_sales: u64
}
EconomyQueuesResponse {
  loot_queue_len: u32
  vendor_backlog_len: u32
}

VendorRoute {
  id: string
  zone: string
  npc_name: string
  path_notes: string
  item_categories: string[]
  enabled: boolean
}
```

### UI shape

- Header strip: paused pill · active cycles chips · `[Pause|Resume]`
- Left panel: vendor route table — zone · npc · categories (chips) · enabled toggle · edit/delete; `[+ New route]` drawer form
- Right panel: KPI stack (plat/hr, items distributed, vendor sales) + queue bars (loot queue, vendor backlog)

---

## 5. Characters — per-character class tuning

**Purpose.** Operator tunes each character: class/role, heal/mana/nuke thresholds, rotation order, class-specific params (CH chain timing, DoT overlap, burn/slow triggers), auto-rez, group override, window title, reward automation, tribute preferences.
**Frequency.** Set-once per character, revisited weekly for tuning.
**UX.** Master list of characters → detail panel with grouped form sections.
**Source.** `textquest-web/src/api.rs::CharacterConfig`.

### Routes

```
GET  /characters                    → HashMap<string, CharacterConfig>
PUT  /characters/{name}             → CharacterConfigUpdate
POST /characters/copy               → ConfigCopyRequest (subset: Rotation | ClassParams | Both)
```

### Shapes

```ts
CharacterConfig {
  character_name: string
  class: string
  role: string
  heal_at_pct: u8           // 0..100
  mana_sit_pct: u8
  nuke_at_pct: u8
  rotation: RotationEntry[]
  class_params: ClassParams
  auto_rez: AutoRezConfig
  group_override: boolean
  group_name?: string
  window_title_format: string
  reward_automation: RewardAutomationConfig
  tribute_preferences: TributePreferences
  tribute_status: TributeStatus
}

RotationEntry { id: string, name: string, priority: u32, enabled: boolean }
ClassParams {
  ch_chain_timing_ms?: u32
  dot_overlap_pct?: u8
  burn_at_hp_pct?: u8
  slow_at_hp_pct?: u8
}
TributePreferences {
  auto_activate: boolean
  warning_threshold_secs: u64
  preferred_tributes: string[]
}
```

### UI shape

- Left: character list (search, filter by class/role)
- Right: detail page with sections
  - Identity: name, class, role (dropdowns)
  - Thresholds: three sliders (heal/mana/nuke %), mono numeric readout
  - Rotation: drag-reorderable list, priority inferred from order, toggle per entry
  - Class Params: four numeric inputs (only relevant ones by class — conditional)
  - Auto-Rez: toggle + config (AutoRezConfig fields TBD — see textquest_common::ipc)
  - Group: override toggle + group name
  - Tribute: auto-activate toggle, warning threshold, preferred tributes list

---

## Priority build order

1. **Sessions** — highest daily use, simplest shapes, read/write paths exist
2. **Characters** — needed before groups are useful (role assignments depend on char config)
3. **Loot** — complex tabs, but heaviest-use config after sessions
4. **Auto-Group** — depends on character list
5. **Economy** — vendor routes + ledger, can ship without queues tab

## Known gaps

- **Raid config** — backend stub: `/api/raid/config` returns 503 `raid_config_unavailable`. Do not build raid UI yet; file a backend issue first.
- **Enum variants marked "confirm"** — some enums (`FilterAction`, `DistributionMethod`, `GroupRole`) were grepped from struct definitions but full variant lists need a second pass on their source files before hard-coding dropdown options.
- **Shared types** — `AutoRezConfig`, `RewardAutomationConfig` live in `textquest_common::ipc` and `shared_character_config`; their fields aren't captured here yet.
