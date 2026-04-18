# TextQuest SDK

A comprehensive TypeScript SDK for the TextQuest web API with full type support and multiple HTTP backend options.

## Features

- **Full Type Safety**: Complete TypeScript type definitions for all API endpoints and responses
- **Multiple HTTP Backends**: Support for both fetch (built-in) and axios
- **API Token Authentication**: Built-in support for API token-based authentication
- **Comprehensive Endpoint Coverage**: Wraps all available TextQuest web API endpoints
- **Error Handling**: Custom error classes with status code tracking
- **No Heavy Dependencies**: Fetch support is built-in, axios is optional

## Installation

```bash
npm install textquest-sdk
```

### With Optional Axios Support

```bash
npm install textquest-sdk axios
```

## Quick Start

### Using Fetch (Default)

```typescript
import { TextQuestClient } from "textquest-sdk";

const client = new TextQuestClient({
  baseUrl: "http://localhost:3001",
  // apiToken: "your-api-token", // optional
});

// Check API health
const health = await client.health();
console.log(health);

// Get active sessions
const sessions = await client.listSessions();
console.log(sessions.sessions);

// Get economy settings
const settings = await client.getEconomySettings();
console.log(settings);
```

### Using Axios

```typescript
import { TextQuestClient } from "textquest-sdk";

const client = new TextQuestClient({
  baseUrl: "http://localhost:3001",
  httpClient: "axios",
  timeout: 30000, // optional, in milliseconds
});

const health = await client.health();
```

## API Documentation

### Health & Status

```typescript
// Check API health
const health = await client.health();
// Returns: { status: string, version: string }
```

### Sessions

```typescript
// List all active sessions
const response = await client.listSessions();
// Returns: { sessions: Session[] }
```

### Accounts

```typescript
// List all accounts
const accounts = await client.listAccounts();

// Get a specific account
const account = await client.getAccount("account-id");

// Create a new account
const newAccount = await client.createAccount({
  username: "newuser",
  expansion_level: 32,
});

// Update an account
const updated = await client.updateAccount("account-id", {
  expansion_level: 33,
});

// Delete an account
await client.deleteAccount("account-id");
```

### Economy Management

```typescript
// Get economy settings
const settings = await client.getEconomySettings();

// Update economy settings
const updated = await client.updateEconomySettings({
  vendor_target_stock: 150,
  krono_target: 20,
});

// Manage vendor routes
const routes = await client.listVendorRoutes();

const route = await client.createVendorRoute({
  name: "Bazaar Loop",
  waypoints: ["Bazaar", "Overthere", "PoK"],
  priority: 1,
});

const updated = await client.updateVendorRoute("route-1", {
  priority: 2,
});

await client.deleteVendorRoute("route-1");

// Check wealth
const wealth = await client.getWealth();
// Returns: { total_krono: number, total_plat: number, by_character: {...} }
```

### Loot Management

```typescript
// Get loot rules
const rules = await client.getLootRules();

// Update loot rules
await client.updateLootRules([
  {
    id: "rule-1",
    name: "Caster Gear",
    pattern: ".*silk.*",
    priority: 1,
    action: "need",
    enabled: true,
  },
]);

// Get/update master looter
const ml = await client.getMasterLooter();

await client.updateMasterLooter({
  enabled: true,
  auto_split: true,
  split_method: "dkp",
});

// Get loot history
const history = await client.getLootHistory();
```

### Character Configuration

```typescript
// List all character configs
const configs = await client.listCharacterConfigs();

// Update a character's config
const updated = await client.updateCharacterConfig("Mage1", {
  strategy_class: "EvocationMage",
  auto_combat: true,
  auto_buff: true,
});

// Get/update auto-accept settings
const autoAccept = await client.getAutoAcceptSettings();

await client.updateAutoAcceptSettings({
  auto_accept_groups: true,
  auto_accept_raids: false,
});
```

### Soul Audit & Monitoring

```typescript
// List soul states
const souls = await client.listSoulStates();

// Get specific character's soul state
const soul = await client.getSoulState("Mage1");

// Get audit history
const audit = await client.getAllAudit();

// Get character-specific audit
const charAudit = await client.getCharacterAudit("Mage1");

// Export audit as CSV
const csv = await client.exportAllAuditCsv();
const charCsv = await client.exportCharacterAuditCsv("Mage1");
```

### Spawn Alerts

```typescript
// List spawn alerts
const alerts = await client.listSpawnAlerts();

// Get spawn alert stats
const stats = await client.getSpawnAlertStats();

// Manage spawn alert configuration
const config = await client.getSpawnAlertConfig();

await client.updateSpawnAlertConfig({
  enabled: true,
  alert_discord: true,
  min_difficulty_tier: 2,
});

// Manage watch list
const watchList = await client.getSpawnAlertWatchList();

await client.addSpawnAlertWatchPattern("Ancient Dragon");
await client.removeSpawnAlertWatchPattern("Ancient Dragon");

// Clear all alerts
await client.clearSpawnAlerts();
```

### Chat Pattern Rules

```typescript
// List all rules
const rules = await client.listChatPatternRules();

// Get rule statistics
const stats = await client.getChatPatternRuleStats();

// Get a specific rule
const rule = await client.getChatPatternRule("rule-1");

// Create or update a rule
const updated = await client.updateChatPatternRule("rule-1", {
  pattern: "LFG",
  response: "Looking for group",
  enabled: true,
  priority: 1,
  cooldown_secs: 60,
});

// Toggle a rule
await client.toggleChatPatternRule("rule-1");

// Manage cooldowns
await client.resetChatPatternRuleCooldown("rule-1");
await client.resetAllChatPatternRuleCooldowns();

// Import multiple rules
await client.importChatPatternRules([
  { pattern: "LFG", response: "Looking for group" },
  { pattern: "LFM", response: "Looking for members" },
]);

// Delete a rule
await client.deleteChatPatternRule("rule-1");
```

### Kill Tracker

```typescript
// Get kill tracker data
const kills = await client.getKillTracker();

// Get kill statistics
const stats = await client.getKillTrackerStats();
// Returns: { total_kills, kills_by_zone, total_loot_value, average_kill_value }
```

### Alerting Configuration

```typescript
// Get alerting configuration
const config = await client.getAlertingConfig();

// Update alerting configuration
const updated = await client.updateAlertingConfig({
  enable_discord: true,
  discord_webhook_url: "https://discord.com/api/...",
  enable_email: false,
  thresholds: {
    death_alert: true,
    memory_warning_mb: 500,
    ipc_latency_warning_ms: 1000,
  },
});

// List operational alerts
const alerts = await client.listOperationalAlerts();

// Acknowledge an alert
await client.acknowledgeAlert(1, "admin");
```

### XAssist Configuration

```typescript
// List all xassist configs
const configs = await client.listXAssistConfigs();

// Get a character's xassist config
const config = await client.getXAssistConfig("Mage1");

// Update xassist config
const updated = await client.updateXAssistConfig("Mage1", {
  enabled: true,
  auto_assist_on_follow: true,
  assist_target: "Tank",
});

// Delete xassist config
await client.deleteXAssistConfig("Mage1");
```

### Additional Endpoints

```typescript
// Box Chat Settings
const boxChat = await client.getBoxChatSettings();
await client.updateBoxChatSettings({
  enabled: true,
  host: "localhost",
  port: 4242,
  auto_connect: true,
});

// Chat Log Settings
const chatLog = await client.getChatLogSettings();
await client.updateChatLogSettings({
  enabled: true,
  path: "./logs/chat.txt",
  rotate_daily: true,
});

// Timestamp Configuration
const timestamps = await client.listTimestampConfigs();
const charTimestamps = await client.getTimestampConfig("Mage1");
await client.updateTimestampConfig("Mage1", {
  format: "24h",
  include_timestamps: true,
});

// Player Watch Configuration
const playerWatch = await client.getPlayerWatchConfig();
await client.updatePlayerWatchConfig({
  enabled: true,
  watch_list: ["GuildName"],
  exclude_own_group: true,
});

// GM Alerts
const gmAlerts = await client.getGmAlerts();

// Say Detection
const sayDetection = await client.getSayDetectionConfig();
```

## Error Handling

The SDK provides a custom `TextQuestClientError` class for error handling:

```typescript
import { TextQuestClient, TextQuestClientError } from "textquest-sdk";

const client = new TextQuestClient({ baseUrl: "http://localhost:3001" });

try {
  const health = await client.health();
} catch (error) {
  if (error instanceof TextQuestClientError) {
    console.error(`API Error: ${error.message}`);
    console.error(`Status Code: ${error.statusCode}`);
  } else {
    console.error("Unexpected error:", error);
  }
}
```

## Type Definitions

The SDK exports a `Types` namespace with all type definitions:

```typescript
import { Types } from "textquest-sdk";

const config: Types.EconomySettings = {
  vendor_target_stock: 100,
  vendor_restock_interval_mins: 60,
  krono_target: 10,
  tradeskill_priority: ["Brewing"],
  auto_harvest: true,
};

const route: Types.VendorRoute = {
  id: "route-1",
  name: "Bazaar Loop",
  waypoints: ["Bazaar"],
  priority: 1,
  enabled: true,
  created_at: new Date().toISOString(),
  last_updated: new Date().toISOString(),
};
```

## Configuration Options

```typescript
interface TextQuestClientConfig {
  // Base URL of the TextQuest API (required)
  baseUrl: string;

  // Optional API token for authentication
  apiToken?: string;

  // HTTP client to use: "fetch" (default) or "axios"
  httpClient?: "fetch" | "axios";

  // Request timeout in milliseconds (default: 30000)
  timeout?: number;
}
```

## Testing

The SDK includes comprehensive tests. To run them:

```bash
npm test
```

To run tests in watch mode:

```bash
npm run test:watch
```

## Building

To build the SDK for distribution:

```bash
npm run build
```

This generates:
- `dist/client.js` - Main client class
- `dist/types.js` - Type definitions
- `dist/*.d.ts` - TypeScript declaration files

## License

MIT
