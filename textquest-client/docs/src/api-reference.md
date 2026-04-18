# API Reference

This document describes the HTTP and WebSocket API endpoints provided by the TextQuest web server.

## Base URL

```
http://localhost:3001
```

**WebSocket URL**: `ws://localhost:3001/ws`

## Authentication

All endpoints support optional token authentication via the `X-API-Token` header:

```bash
curl -H "X-API-Token: your-token" http://localhost:3001/api/health
```

Set `TEXTQUEST_API_TOKEN` environment variable to enable auth on the server.

## Endpoints

### Health Check

#### GET /api/health

Returns server health status.

**Response**: 200 OK

```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

---

### Sessions

#### GET /api/sessions

List all active sessions.

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

### Session Control

#### PUT /api/control/pause/:session_id

Pause a session.

**Response**: 200 OK

#### PUT /api/control/resume/:session_id

Resume a paused session.

**Response**: 200 OK

#### PUT /api/control/group/:session_id

Set session group assignment.

```json
{
  "group_id": 1
}
```

**Response**: 200 OK

#### PUT /api/control/broadcast-all/:session_id

Set session to receive all broadcast commands.

**Response**: 200 OK

---

### Command Relay

#### POST /api/command

Relay a slash command to sessions.

```json
{
  "command": "/target frostreaver",
  "target": "Frostreaver"
}
```

**Response**: 200 OK

```json
{
  "success": true,
  "message": "Command '/target frostreaver' relayed"
}
```

---

### Character Config

#### GET /api/config/characters

List character configs.

**Response**: 200 OK

#### PUT /api/config/characters/:name

Upsert character config.

**Request**: `CharacterConfig` object

**Response**: 200 OK

---

### Economy

#### GET /api/economy/settings

Get economy settings.

**Response**: 200 OK

#### PUT /api/economy/settings

Update economy settings.

**Request**: `EconomySettings` object

**Response**: 204 No Content

---

### WebSocket Events

Connect to `ws://localhost:3001/ws` for real-time session updates.

**Authentication**: Pass `token` query param: `ws://localhost:3001/ws?token=your-token`

**Event Types**:

- `session_update`: Session state change
- `command`: Command execution result
- `error`: Error condition