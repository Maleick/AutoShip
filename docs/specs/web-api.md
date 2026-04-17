# Web API Specification

## Overview

TextQuest provides a browser-based dashboard via a REST API + WebSocket interface built with Axum and React. The API serves configuration, state, and control surfaces for multibox orchestration.

**Base URL**: `http://localhost:8080` (configurable)

**API Prefix**: `/api`

## Authentication

Currently **no authentication** is implemented. All endpoints are accessible without credentials.

**Future**: Consider bearer token or API key authentication for production deployments.

## HTTP Status Codes

| Code | Meaning                     |
| ---- | --------------------------- |
| 200  | Success                     |
| 201  | Resource created            |
| 204  | Success (no content)        |
| 400  | Bad request (invalid input) |
| 404  | Not found                   |
| 501  | Not implemented             |
| 5xx  | Server error                |

## Error Response Format

All error responses use the same JSON schema:

```json
{
  "error": "Human-readable error message"
}
```

**Example**:

```json
{
  "error": "Character configuration API is not implemented in this build"
}
```

## Core Endpoints

### Health Check

#### GET /api/health

Check API server health and version.

**Response**: 200 OK

```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

**Purpose**: Liveness probe for deployment health checks.

---

## Sessions API

### List Active Sessions

#### GET /api/sessions

Retrieve all active character sessions currently under orchestration.

**Response**: 200 OK

```json
[
  {
    "client_id": 1,
    "character_name": "Frostreaver",
    "zone": "South Karana",
    "level": 60,
    "hp_pct": 95.5,
    "mana_pct": 87.0,
    "status": "idle"
  },
  {
    "client_id": 2,
    "character_name": "Noxus",
    "zone": "North Karana",
    "level": 58,
    "hp_pct": 100.0,
    "mana_pct": 0.0,
    "status": "combat"
  }
]
```

**Schema**: `SessionInfo`

- `client_id: u32` — Process ID or session identifier
- `character_name: String` — Character name
- `zone: String` — Current zone name
- `level: u8` — Character level
- `hp_pct: f32` — HP percentage (0-100)
- `mana_pct: f32` — Mana percentage (0-100)
- `status: String` — Current status (idle, combat, moving, dead, etc.)

**Notes**:

- In current implementation, zone/level/hp/mana/status are filled from character configs (demo data).
- Future: Pull live state from shared memory or IPC.

---

## Character Configuration API

### List Character Configs

#### GET /api/config/characters

Retrieve all character ability and behavior configurations.

**Response**: 200 OK

```json
[
  {
    "character_name": "Frostreaver",
    "class": "Cleric",
    "role": "Healer",
    "heal_at_pct": 70,
    "mana_sit_pct": 25,
    "nuke_at_pct": 90,
    "rotation": [
      {
        "id": "complete_heal",
        "name": "Complete Heal",
        "priority": 1,
        "enabled": true
      },
      {
        "id": "celestial_healing",
        "name": "Celestial Healing",
        "priority": 2,
        "enabled": true
      }
    ],
    "class_params": {
      "ch_chain_timing_ms": 2500,
      "dot_overlap_pct": null,
      "burn_at_hp_pct": null,
      "slow_at_hp_pct": null
    },
    "group_override": false,
    "group_name": "Group 1",
    "reward_automation": {
      "rules": [
        {
          "task_matcher": "*",
          "preference": {
            "kind": "by_position",
            "reward_position": 1
          }
        }
      ]
    }
  }
]
```

**Schema**: `CharacterConfig`

- `character_name: String` — Character name (keyed)
- `class: String` — Class (Cleric, Warrior, Wizard, etc.)
- `role: String` — Combat role (Healer, Tank, DPS, Support)
- `heal_at_pct: u8` — Heal trigger threshold (%)
- `mana_sit_pct: u8` — Mana sit threshold (%)
- `nuke_at_pct: u8` — Nuke trigger threshold (%)
- `rotation: Vec<RotationEntry>` — Rotation abilities
- `class_params: ClassParams` — Class-specific tuning
- `group_override: bool` — Override group assignment
- `group_name: Option<String>` — Group assignment
- `reward_automation: RewardAutomationConfig` — Per-task reward selection rules

`RewardAutomationConfig.rules` supports:

- `task_matcher: String` — Reward window title match. `*` acts as the default
  fallback rule.
- `preference.kind = "by_name"` with `reward_name`
- `preference.kind = "by_position"` with 1-based `reward_position`

### Upsert Character Config

#### PUT /api/config/characters/:name

Create or update a character configuration.

**Path Parameters**:

- `name: String` — Character name

**Request Body**: `CharacterConfig` (character_name field ignored, uses path param)

**Response**: 200 OK

```json
{
  "character_name": "Frostreaver",
  "class": "Cleric",
  ...
}
```

---

## Economy Configuration API

### Get Economy Settings

#### GET /api/economy/settings

Retrieve full economy configuration (krono farming, banking, tradeskills).

**Response**: 200 OK

```json
{
  "krono": {
    "target_rate_per_day": 5,
    "min_sell_price": 900,
    "max_buy_price": 850,
    "restock_threshold": 3,
    "enabled": true
  },
  "banking_rules": [
    {
      "id": "default-bank",
      "item_category": "Tradeskill",
      "deposit_threshold": 100,
      "keep_on_hand": 20,
      "auto_deposit": true
    }
  ],
  "tradeskill_supplies": [
    {
      "id": "pottery",
      "skill": "Pottery",
      "materials": ["Clay", "Water"],
      "restock_quantity": 50,
      "source_zone": "South Karana",
      "enabled": true
    }
  ]
}
```

**Schema**: `EconomySettings`

- `krono: KronoSettings`
  - `target_rate_per_day: u32` — Daily krono target
  - `min_sell_price: u32` — Minimum platinum to sell krono
  - `max_buy_price: u32` — Maximum platinum to buy krono
  - `restock_threshold: u32` — Restock when quantity drops below this
  - `enabled: bool` — Krono farming enabled/disabled
- `banking_rules: Vec<BankingRule>`
- `tradeskill_supplies: Vec<TradeskillSupply>`

### Update Economy Settings

#### PUT /api/economy/settings

Update full economy configuration.

**Request Body**: `EconomySettings`

**Response**: 204 No Content

**Notes**:

- In current implementation, settings are not persisted (demo mode).
- Future: Write to config file or database.

---

## Vendor Routes API

### List Vendor Routes

#### GET /api/economy/vendor-routes

Retrieve all configured vendor routes for farming/trading.

**Response**: 200 OK

```json
[
  {
    "id": "vr-qey",
    "zone": "Queynos Hills",
    "npc_name": "Merchant",
    "path_notes": "Near the stone",
    "item_categories": ["Armor", "Weapons"],
    "enabled": true
  },
  {
    "id": "vr-pok",
    "zone": "Plane of Knowledge",
    "npc_name": "Tradeskill Master",
    "path_notes": "Central tower",
    "item_categories": ["Tradeskill"],
    "enabled": true
  }
]
```

**Schema**: `VendorRoute`

- `id: String` — Unique route ID (e.g., "vr-qey")
- `zone: String` — Zone name
- `npc_name: String` — NPC name to trade with
- `path_notes: String` — Navigation notes (camp spot, relative coords)
- `item_categories: Vec<String>` — Item categories to buy/sell
- `enabled: bool` — Route active/inactive

### Create Vendor Route

#### POST /api/economy/vendor-routes

Create a new vendor route.

**Request Body**: `VendorRoute` (id may be ignored; server assigns if empty)

**Response**: 201 Created

```json
{
  "id": "vr-new",
  "zone": "The Bazaar",
  "npc_name": "Merchant Lord",
  "path_notes": "Central area",
  "item_categories": ["Rare"],
  "enabled": true
}
```

### Update Vendor Route

#### PUT /api/economy/vendor-routes/:id

Update an existing vendor route.

**Path Parameters**:

- `id: String` — Route ID

**Request Body**: `VendorRoute` (id field in URL takes precedence)

**Response**: 200 OK

```json
{
  "id": "vr-qey",
  "zone": "Queynos Hills",
  ...
}
```

### Delete Vendor Route

#### DELETE /api/economy/vendor-routes/:id

Delete a vendor route.

**Path Parameters**:

- `id: String` — Route ID

**Response**: 204 No Content

---

## Wealth & Economy Metrics

### Get Wealth History

#### GET /api/economy/wealth

Retrieve current wealth snapshot and historical trend.

**Response**: 200 OK

```json
{
  "current": {
    "timestamp": "2024-12-20T12:00:00Z",
    "plat": 250000,
    "krono": 15,
    "item_value_estimate": 500000
  },
  "snapshots": [
    {
      "timestamp": "2024-12-20T11:00:00Z",
      "plat": 245000,
      "krono": 15,
      "item_value_estimate": 495000
    },
    {
      "timestamp": "2024-12-20T10:00:00Z",
      "plat": 240000,
      "krono": 14,
      "item_value_estimate": 490000
    }
  ]
}
```

**Schema**: `WealthHistory`

- `current: WealthSnapshot` — Current wealth state
- `snapshots: Vec<WealthSnapshot>` — Historical snapshots

**WealthSnapshot** fields:

- `timestamp: String` — ISO 8601 timestamp
- `plat: u64` — Platinum currency
- `krono: u32` — Krono tokens
- `item_value_estimate: u64` — Estimated value of inventory items

**Notes**:

- In current implementation, history is demo data (two snapshots).
- Future: Pull from SQLite metrics database (daily/hourly snapshots).

---

## Raid Configuration API

### Raid Config Endpoints

#### GET /api/raid/config

**Status**: 501 Not Implemented (placeholder)

**Response**: 501 Not Implemented

```json
{
  "error": "Raid configuration API is not implemented in this build"
}
```

#### PUT /api/raid/config

**Status**: 501 Not Implemented (placeholder)

**Response**: 501 Not Implemented

---

## Loot Management

### Loot Module (Demo)

Placeholder routes for loot tracking and distribution.

**Endpoints**: To be documented when implemented.

---

## Soul & Automation (Future)

### Soul Module (Demo)

Placeholder routes for soul engine state and automation control.

**Endpoints**: To be documented when implemented.

---

## WebSocket Events (Future)

### Event Stream

**URL**: `ws://localhost:8080/api/events` (not yet implemented)

Planned to stream real-time state updates:

- Character position/status changes
- Combat state transitions
- Loot notifications
- Economy alerts

---

## Request/Response Examples

### Example: Update Economy Settings

```bash
curl -X PUT http://localhost:8080/api/economy/settings \
  -H 'Content-Type: application/json' \
  -d '{
    "krono": {
      "target_rate_per_day": 3,
      "min_sell_price": 950,
      "max_buy_price": 800,
      "restock_threshold": 2,
      "enabled": false
    },
    "banking_rules": [],
    "tradeskill_supplies": []
  }'
```

**Response**: 204 No Content

### Example: Create Vendor Route

```bash
curl -X POST http://localhost:8080/api/economy/vendor-routes \
  -H 'Content-Type: application/json' \
  -d '{
    "id": "vr-bazaar",
    "zone": "The Bazaar",
    "npc_name": "Merchant",
    "path_notes": "Center plaza",
    "item_categories": ["Gems", "Rare"],
    "enabled": true
  }'
```

**Response**: 201 Created

```json
{
  "id": "vr-bazaar",
  "zone": "The Bazaar",
  "npc_name": "Merchant",
  "path_notes": "Center plaza",
  "item_categories": ["Gems", "Rare"],
  "enabled": true
}
```

### Example: List Sessions

```bash
curl http://localhost:8080/api/sessions
```

**Response**: 200 OK

```json
[
  {
    "client_id": 1,
    "character_name": "Frostreaver",
    "zone": "South Karana",
    "level": 60,
    "hp_pct": 95.5,
    "mana_pct": 87.0,
    "status": "idle"
  }
]
```

---

## Demo Data

The API server initializes with demo data to support UI development without live EQ:

- **Sessions**: 5 sample characters (Frostreaver, Noxus, Aelrindel, Grok, Valerius)
- **Economy Settings**: Default krono/banking config
- **Vendor Routes**: 2 sample routes (Queynos Hills, Plane of Knowledge)
- **Wealth History**: 3 snapshots spanning 2 hours

When connected to a live orchestrator, all endpoints would read from shared memory or IPC.

---

## CORS & Browser Access

Currently, CORS is **not configured**. To run the React SPA locally:

1. Dev server: `yarn start` (runs on port 3000)
2. Proxy requests to `http://localhost:8080/api`

For production, configure CORS headers:

```
Access-Control-Allow-Origin: <allowed-origins>
Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS
Access-Control-Allow-Headers: Content-Type, Authorization
```

---

## Pagination & Filtering (Future)

Planned for list endpoints:

- `?limit=50` — Limit results
- `?offset=100` — Pagination offset
- `?filter=enabled` — Filter by property
- `?sort=timestamp` — Sort by field

---

## Rate Limiting (Future)

Planned rate limiting policy:

- Per-IP: 100 requests/minute (configurable)
- Per-character: 10 state changes/second
- Burst: Allow 5 burst requests, then throttle

---

## Version Compatibility

### Current

- **API Version**: v1 (implicit, no version header)
- **Breaking Changes**: None planned before v2

### Future Versioning

- Add `X-API-Version: 1` response header
- Support `/api/v2/...` paths alongside `/api/v1/...`
- Deprecation period: 6 months before removing old versions

---

## References

- API handlers: `textquest-web/src/api.rs`
- Sub-modules:
  - `textquest-web/src/api/economy.rs`
  - `textquest-web/src/api/loot.rs`
  - `textquest-web/src/api/soul.rs`
- Server setup: `textquest-web/src/main.rs`
- React SPA: `textquest-web/web/src/`
