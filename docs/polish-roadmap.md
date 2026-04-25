# Polish Roadmap: TUI & Web Dashboard Complete Breakdown

**Status**: Documentation of complete polish initiative structure mapping all work into epics, issues, and milestones for organized execution.

**Last Updated**: 2026-04-25

---

## Overview

This document defines the complete Polish Initiative — a multi-phase effort to enhance the TextQuest TUI and Web Dashboard with user-facing features, real-time monitoring, accessibility, and advanced in-game integration capabilities.

**Initiative Scope**:
- **1 Master Epic** — Polish Initiative (cross-cutting coordination)
- **6 Major Epics** — Thematic groupings (TUI, Web Dashboard, Overlays, Debugger)
- **12 Features** — User-facing capabilities
- **6 Tasks** — Implementation details
- **28 Total Issues** — Structured decomposition
- **7 Milestones** — Sequential execution phases

---

## Milestone Map & Execution Order

| Phase | Milestone | Goal | Status |
|-------|-----------|------|--------|
| 1 | TUI Polish Phase 1 | Quick wins, foundational UX | Planned |
| 2 | TUI Polish Phase 2 | Real-time monitoring & events | Planned |
| 3 | Debugger & Packet Tools | Test branch integration | Planned |
| 4 | Web Dashboard Phase 1 | Frontend foundation & build setup | Planned |
| 5 | Web Dashboard Phase 2 | Configuration UIs | Planned |
| 6 | Web Dashboard Phase 3 | Session monitoring & real-time updates | Planned |
| 7 | In-Game Overlay System | Direct3D rendering & in-game windows | Planned |

---

## TUI Polish Phase 1: Quick Wins

**Goal**: Foundational UX improvements with high impact, low effort.

### Epic #970: TUI Polish & User Experience

#### Help System Track

**#971 Feature**: In-Game Help & FAQ System
- Accessible in-game help content tied to game context
- Searchable FAQ system for common player questions
- Integration with TUI navigation

Sub-issues:
- **#972 Task**: Help data structure & loading
  - Define help data schema (markdown or structured format)
  - Implement help resource loader
  - Handle versioning and updates
  
- **#973 Task**: Help search & filter implementation
  - Full-text search across help topics
  - Category/tag-based filtering
  - Real-time search relevance scoring
  
- **#974 Task**: TUI help panel UI & navigation
  - Help overlay panel design
  - Keyboard navigation (Tab, arrows, Enter)
  - Context-aware help suggestions

#### Diagnostics & Errors

**#993 Feature**: Enhanced TUI Error Messages & Diagnostics
- **Scope**: Connection health, IPC diagnostics, memory/resource monitoring
- **Components**:
  - Diagnostic panels (connection status, IPC queue depth, memory usage)
  - Clear error messages with actionable suggestions
  - Error code tracking and resolution hints
  - Automatic diagnostic capture for troubleshooting

#### Theme & Accessibility

**#994 Feature**: TUI Theme System & Customization
- Multiple built-in themes (dark, light, high-contrast)
- Per-character theme overrides
- Theme persistence
- Color palette customization

**#997 Feature**: TUI Keyboard Shortcuts & Accessibility
- Tab/arrow navigation for all UI elements
- Vi/Emacs keybinding support
- Screen reader compatibility (ARIA labels, semantic markup)
- High-contrast mode for vision accessibility

---

## TUI Polish Phase 2: Monitoring & Events

**Goal**: Real-time visibility and event-driven alerts.

### Epic #970 (Continued): Performance & Events

#### Performance Monitoring

**#975 Feature**: Performance Monitoring Dashboard
- Real-time metrics collection and display
- Historical trend visualization
- Configurable alert thresholds

Sub-issues:
- **#976 Task**: Metrics aggregation & real-time collection
  - Define metrics schema (DPS, movement, loot, system resources)
  - Implement producer (game hook → metrics queue)
  - Time-series storage and rotation
  
- **#977 Task**: Metrics TUI dashboard panel
  - Real-time gauge displays
  - Trend mini-charts
  - Alert status indicators

#### Hotkeys & Audio

**#989 Epic**: Hotkey & Audio Alert Systems

**#990 Feature**: Hotkey & Slash Command Registration
- Register custom hotkeys from configuration
- Hotkey binding with modifier support (Ctrl, Shift, Alt)
- Conflict detection and warnings
- Hotkey profiles per character

**#991 Feature**: Audio Alert System & Event Routing
- Event-triggered audio playback
- Multiple audio channels (alerts, notifications, ambient)
- Volume control per channel
- Custom sound upload support
- Audio file format support (WAV, MP3, OGG)

---

## Web Dashboard Phase 1: Foundation

**Goal**: Build frontend infrastructure and setup.

### Epic #978: Web Dashboard Foundation & Implementation

#### Frontend Infrastructure

**#979 Feature**: Web Dashboard Frontend Build Setup
- **Tech Stack**: Vite + React + TypeScript + Tailwind CSS
- **Deliverables**:
  - Project scaffold and tooling
  - Development server with hot reload
  - Production build optimization
  - API proxy for backend integration
  - TypeScript configuration for strict mode

**Included Components**:
- Layout system (header, sidebar, main content area, footer)
- Navigation (breadcrumbs, sidebar menu, top navigation)
- Form components (text input, select, checkbox, radio, textarea)
- Modal/dialog system (confirmation, form modals, overlays)
- Status indicators (badges, spinners, progress bars)

---

## Web Dashboard Phase 2: Configuration

**Goal**: User-facing configuration UIs.

### Epic #978 (Continued)

#### Credential Management

**#981 Feature**: Web Dashboard Credential Management UI
- **Scope**: Account CRUD, character mapping, launch sequence setup, secure password handling
- **Components**:
  - Account list with search/filter
  - Add/edit account modal with validation
  - Character assignment grid
  - Launch sequence editor (drag-drop ordering)
  - Password field with visibility toggle
  - Test credential verification (connection test)
  - Secure credential storage integration

#### Group & Camp Configuration

**#983 Feature**: Web Dashboard Group & Camp Configuration UI
- **Scope**: Group creation/editing, character assignment, role assignment, camp settings, pull strategy
- **Components**:
  - Group list and creation form
  - Character assignment interface
  - Class role selector (Tank, Healer, DPS, Support)
  - Camp settings visual editor:
    - Position coordinates
    - Radius and formation
    - Pull range and targeting
  - Pull strategy selector (safe pull, aggressive, tactical)
  - Camp template save/load system

---

## Web Dashboard Phase 3: Monitoring

**Goal**: Real-time operational visibility.

### Epic #978 (Continued)

#### Session Monitoring

**#986 Feature**: Web Dashboard Session Monitoring
- **Real-Time Components**:
  - Live client status grid (grid view of all active clients)
  - Per-client health/mana bars with percentages
  - Zone name and camp phase display
  - Stuck detection indicator with timestamps
  - Action buttons (restart, pause, resume, kick client)
  - WebSocket real-time updates (<1s latency)
  - Event alert feed (kills, deaths, loot, phase changes)

#### Additional Features (Phase 2+)

- Log viewer with filtering and search
- Dark/light theme system
- Responsive mobile layout
- Multi-group overview dashboard
- Export/reporting (session logs, metrics snapshots)

---

## In-Game Overlay System

**Goal**: Direct EQ integration for in-game UI.

### Epic #987: In-Game Overlay GUI Windows

#### Core Rendering

**#988 Feature**: Direct3D Overlay Rendering System
- **Scope**: D3D 11/12 hook and overlay rendering pipeline
- **Components**:
  - Direct3D device hooking (11 and 12 support)
  - ImGui integration or custom widget library
  - Overlay texture management and batching
  - Device lost/reset handling
  - Ultra-low performance budget (<1ms frame time)
  - Transparent overlay blending

#### Window Management & Input (Phase 1.5)

- Draggable and resizable windows
- Z-order and focus management
- Input handling (mouse cursor, keyboard events)
- Per-character position persistence
- Theme support for overlay windows

#### Core Windows (Phase 2)

Planned in-game overlay windows:
- Spell Loadout window (current loaded spells, hotkey bindings)
- Rotation queue display (next N spells in rotation)
- Pull status & target list (current target, adds)
- Force Target override (manual target selection)
- Camp status & controls (group position, phase, action buttons)

---

## Debugger & Packet Tools

**Goal**: Integrate testing and debugging tools.

### Epic #995: Debugger & Packet Tools Integration

#### Debugger Integration

**#996 Task**: Extract & Integrate Debugger from Test Branch
- **Scope**: Extract debugger implementation from test branch, clean up test-specific code
- **Steps**:
  1. Remove Test offsets and debug defaults
  2. Wire debug commands to dispatcher
  3. Update documentation and usage examples
  4. Add live validation tasks (offset verification)
  5. Integrate into main CI/CD pipeline

#### Packet Monitor UI (Phase 1.5)

- Packet capture and filtering interface
- Opcode inspection and lookup
- Message routing visualization
- Replay and analysis tools
- Binary/hex dump viewing

#### Tactical Map Enhancements (Phase 2)

- Improved map controls (pan, zoom, rotate)
- Named NPC tracking visualization
- Route planning display (waypoint paths)
- Stuck detection visualization (heatmaps)

---

## Issue Tagging Standards

All issues created as part of this initiative should include:

### By Type
- `feature` — New user-facing capability
- `task` — Implementation detail or subtask
- `epic` — Large thematic grouping
- `documentation` — Guides and reference

### By Category
- `ui` — User interface (TUI or web)
- `web` — Web dashboard specific
- `monitoring` — Real-time metrics and status
- `debugging` — Debug and development tools
- `quality-of-life` — UX improvements
- `accessibility` — Keyboard nav, screen readers, a11y

### By Priority
- `high-priority` — Blocking other work or critical path
- `agent:ready` — Work is well-scoped and ready for autonomous agents

### By Assignee
- (GitHub milestone assignment)
- (Parent epic linkage)

---

## Execution Guidelines

### Phase Sequencing

1. **TUI Polish Phase 1** — Quick wins (help, diagnostics, themes, keyboard)
   - Duration: ~2-3 weeks
   - Owner: TUI agent team
   - Delivery: Merged help system + enhanced errors + accessibility

2. **TUI Polish Phase 2** — Real-time features (performance monitoring, hotkeys, audio)
   - Duration: ~3-4 weeks
   - Owner: TUI + Performance agent teams
   - Delivery: Live metrics dashboard + hotkey system + audio alerts

3. **Debugger & Packet Tools** — Integration and cleanup
   - Duration: ~2 weeks
   - Owner: Debugger team
   - Delivery: Clean test branch extraction + packet monitor UI

4. **Web Dashboard Phase 1** — Frontend infrastructure
   - Duration: ~2-3 weeks
   - Owner: Web frontend team
   - Delivery: Vite scaffold + build setup + core components

5. **Web Dashboard Phase 2** — Configuration UIs
   - Duration: ~4-5 weeks
   - Owner: Web forms team
   - Delivery: Credential + group/camp management UIs

6. **Web Dashboard Phase 3** — Monitoring & real-time
   - Duration: ~3-4 weeks
   - Owner: Web monitoring team
   - Delivery: Live session monitoring + WebSocket integration

7. **In-Game Overlay System** — Advanced integration
   - Duration: ~4-6 weeks
   - Owner: Overlay/D3D team
   - Delivery: Core rendering + window management + spell loadout window

### Quality Checkpoints

- All features must pass automated tests
- Accessibility features (keyboard, screen reader) verified with WCAG 2.1 AA
- Performance benchmarks met (hotkeys <10ms, overlays <1ms)
- Web dashboard API contract validated against backend
- User documentation (in-game help, web tooltips) complete

### Milestone Tracking

- Milestones in GitHub correspond to phases above
- Sub-issues linked to parent feature issues
- `agent:ready` label indicates work is scoped for autonomous dispatch
- Weekly status updates in project memory

---

## Integration Points

### TUI ↔ Web Dashboard
- Shared credential/account model
- Common theme/color system
- Identical keyboard shortcut bindings

### Web Dashboard ↔ Backend
- REST API for configuration CRUD
- WebSocket for real-time session monitoring
- Secure credential storage

### In-Game Overlay ↔ TUI & Web
- Shared camp/group state
- Overlay windows driven by same data model
- Configuration synced across all UIs

---

## Notes

- This document serves as the master decomposition. Individual issues should reference this as context.
- Phases may overlap (e.g., Phase 2 TUI work can begin while Phase 1 is being reviewed).
- Additional gaps may be identified during implementation — these become separate GitHub issues linked to parent epics.
- This roadmap is living documentation — update as priorities shift or scope changes.

