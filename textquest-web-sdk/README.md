# TextQuest Web API Rust SDK

A clean Rust SDK for interacting with the TextQuest web API. Provides both async (tokio-based) and synchronous interfaces for the currently supported REST endpoints.

## Features

- **Supported REST Endpoints**: Covers the currently implemented TextQuest web API routes
- **Async & Sync**: Both async (`Client`) and blocking (`BlockingClient`) interfaces
- **Type-Safe**: Full type definitions for supported API requests and responses with serde serialization/deserialization
- **Error Handling**: Comprehensive error types for API operations
- **Optional Authentication**: Support for API token-based authentication via `X-API-Token` header
- **Actively Evolving**: Suitable for supported integrations today, with additional endpoint coverage to be added over time

## Supported Endpoints

### Health & Info

- `GET /api/health` - API health check

### Sessions

- `GET /api/sessions` - List active sessions

### Chat Settings

- `GET /api/chat-log/settings` - Get chat log settings
- `PUT /api/chat-log/settings` - Update chat log settings
- `GET /api/box-chat/settings` - Get box chat configuration
- `PUT /api/box-chat/settings` - Update box chat configuration

### Character Configuration

- `GET /api/config/characters` - List all character configs
- `PUT /api/config/characters/{character}` - Update character config
- `GET /api/config/auto-accept` - Get auto-accept settings
- `PUT /api/config/auto-accept` - Update auto-accept settings
- `GET /api/config/player-watch` - Get player watch config
- `PUT /api/config/player-watch` - Update player watch config

### Economy

- `GET /api/economy/settings` - Get economy settings
- `PUT /api/economy/settings` - Update economy settings
- `GET /api/economy/vendor-routes` - List vendor routes
- `POST /api/economy/vendor-routes` - Create vendor route
- `PUT /api/economy/vendor-routes/{id}` - Update vendor route
- `DELETE /api/economy/vendor-routes/{id}` - Delete vendor route
- `GET /api/economy/wealth` - Get wealth history

### Loot Management

- `GET /api/loot/rules` - Get loot rules
- `PUT /api/loot/rules` - Update loot rules
- `GET /api/loot/filters` - Get loot filters
- `PUT /api/loot/filters/{character}` - Update character loot filter
- `GET /api/loot/master-looter` - Get master looter assignment
- `PUT /api/loot/master-looter` - Set master looter
- `GET /api/loot/distribution` - Get loot distribution settings
- `PUT /api/loot/distribution` - Update loot distribution
- `GET /api/loot/history` - Get loot history

### Soul (AI/Bot Management)

- `GET /api/soul` - List all soul states
- `GET /api/soul/{character_id}` - Get soul state for character
- `GET /api/soul/audit` - Get audit log
- `GET /api/soul/audit/{character_id}` - Get character audit log

### Alerts

- `GET /api/alerts/config` - Get alerting configuration
- `PUT /api/alerts/config` - Update alerting configuration
- `GET /api/alerts/history` - Get alert history

### Spawn Alerts

- `GET /api/spawn-alerts` - List spawn alerts
- `DELETE /api/spawn-alerts` - Clear all spawn alerts
- `GET /api/spawn-alerts/stats` - Get spawn alert stats
- `GET /api/spawn-alerts/config` - Get spawn alert config
- `PUT /api/spawn-alerts/config` - Update spawn alert config
- `GET /api/spawn-alerts/watch-list` - Get watch list
- `PUT /api/spawn-alerts/watch-list/{pattern}` - Add pattern
- `DELETE /api/spawn-alerts/watch-list/{pattern}` - Remove pattern

### Timestamp Configuration

- `GET /api/timestamp-config` - List timestamp configs
- `GET /api/timestamp-config/{character}` - Get character timestamp config
- `PUT /api/timestamp-config/{character}` - Update character timestamp config

### Kill Tracker

- `GET /api/kill-tracker/history` - Get kill history
- `GET /api/kill-tracker/stats` - Get kill stats

### GM Alerts

- `GET /api/gm-alerts` - Get GM alert status

### Say Detection

- `GET /api/say-detection/config` - Get say detection config
- `PUT /api/say-detection/config` - Update say detection config
- `GET /api/say-detection/matches` - Get say detection matches

### XAssist

- `GET /api/xassist/configs` - List XAssist configs
- `GET /api/xassist/config/{character}` - Get XAssist config
- `PUT /api/xassist/config/{character}` - Update XAssist config
- `DELETE /api/xassist/config/{character}` - Delete XAssist config

### Chat Pattern Rules

- `GET /api/chat-pattern-rules` - List rules
- `GET /api/chat-pattern-rules/stats` - Get stats
- `POST /api/chat-pattern-rules/import` - Import rules
- `GET /api/chat-pattern-rules/{id}` - Get specific rule
- `PUT /api/chat-pattern-rules/{id}` - Update rule
- `DELETE /api/chat-pattern-rules/{id}` - Delete rule
- `PUT /api/chat-pattern-rules/{id}/toggle` - Toggle rule
- `PUT /api/chat-pattern-rules/{id}/reset-cooldown` - Reset cooldown
- `PUT /api/chat-pattern-rules/cooldowns/reset` - Reset all cooldowns

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
textquest-web-sdk = "0.1.0"
tokio = { version = "1", features = ["full"] }
```

## Usage

### Async Example

```rust
use textquest_web_sdk::Client;

#[tokio::main]
async fn main() {
    let client = Client::new("http://localhost:3000");

    // Get health status
    match client.health().await {
        Ok(health) => println!("API Status: {}", health.status),
        Err(e) => eprintln!("Error: {}", e),
    }

    // List sessions
    match client.list_sessions().await {
        Ok(sessions) => {
            for session in sessions {
                println!("{}: Level {}", session.character, session.level);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

### Synchronous Example

```rust
use textquest_web_sdk::BlockingClient;

fn main() {
    let client = BlockingClient::new("http://localhost:3000")
        .expect("Failed to create client");

    // Get health status
    match client.health() {
        Ok(health) => println!("API Status: {}", health.status),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

### With Authentication

```rust
use textquest_web_sdk::Client;

#[tokio::main]
async fn main() {
    let client = Client::with_token(
        "http://localhost:3000",
        Some("your-api-token".to_string())
    );

    // All requests will include X-API-Token header
    if let Ok(configs) = client.list_character_configs().await {
        println!("Found {} character configs", configs.len());
    }
}
```

## Error Handling

The SDK provides a `Result<T>` type and comprehensive `Error` type for handling failures:

```rust
use textquest_web_sdk::{Client, Error};

#[tokio::main]
async fn main() {
    let client = Client::new("http://localhost:3000");

    match client.health().await {
        Ok(health) => println!("OK: {}", health.status),
        Err(Error::Http(msg)) => eprintln!("HTTP error: {}", msg),
        Err(Error::ApiError { status, message }) => {
            eprintln!("API error ({}): {}", status, message)
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## Running Examples

```bash
# Async example
cargo run --example async_example

# Synchronous example
cargo run --example sync_example

# With authentication
TEXTQUEST_API_TOKEN=your-token cargo run --example with_token
```

## Running Tests

```bash
cargo test
```

All tests are synchronous unit tests that verify type serialization/deserialization without requiring a live API server.

## Documentation

Full API documentation is available via:

```bash
cargo doc --open
```

## Performance

- **Async Client**: Uses `reqwest` with tokio for non-blocking I/O
- **Sync Client**: Wraps async client with a tokio runtime (one runtime per client)
- **Connection Pooling**: Automatic connection pooling via reqwest
- **Timeouts**: Configurable per request (currently uses reqwest defaults)

## Contributing

Contributions are welcome! Please ensure:

- All tests pass: `cargo test`
- Code is formatted: `cargo fmt`
- No clippy warnings: `cargo clippy`

## License

MIT

## Security

- **API Token**: When using `TEXTQUEST_API_TOKEN`, ensure it's transmitted over HTTPS in production
- **Credential Storage**: Never hardcode credentials in your code; use environment variables
- **Path Parameters**: Values containing special characters (spaces, `/`, etc.) in character names or IDs may need to be percent-encoded by the caller before passing to the client methods

## Support

For issues and feature requests, visit: https://github.com/Maleick/TextQuest/issues
