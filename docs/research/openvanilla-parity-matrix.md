# OpenVanilla Parity Comparison Matrix

**Last Updated:** 2026-04-25  
**Status:** Research Complete  
**Related Issues:** #819-#856  

## Overview

Comprehensive comparison matrix between OpenVanilla/MQ2 capabilities and TextQuest's current implementation status. This consolidates research from 28 feature gap issues (#819-#856) across seven functional domains.

## Domain Summary

| Domain | Gap Issues | Status |
|--------|-----------|--------|
| Architecture & Extensibility | #819, #820, #822 | 3 gaps identified |
| Character & Ability Management | #828, #831, #833, #834, #852 | 5 gaps identified |
| Inventory & Loot | #824, #825, #826 | 3 gaps identified |
| Multiboxing & Coordination | #836, #838, #851 | 3 gaps identified |
| Monitoring & Analytics | #840, #856, #849 | 3 gaps identified |
| Automation & Utilities | #847, #848, #830, #854, #855 | 5 gaps identified |
| Integration & System | #839, #853 | 2 gaps identified |

---

## Detailed Feature Comparison

### Architecture & Extensibility

| Feature | OpenVanilla | TextQuest | Status | Issue | Notes |
|---------|-------------|----------|--------|-------|-------|
| Plugin system | Yes (Lua-based plugins) | No | Missing | #819 | Community-driven extensibility for custom modules |
| Lua scripting support | Native (full API) | No | Missing | #820 | Direct script execution environment |
| EQBC inter-character communication | Yes (protocol) | No | Missing | #822 | Multi-character coordination via EQBC protocol |

### Character & Ability Management

| Feature | OpenVanilla | TextQuest | Status | Issue | Notes |
|---------|-------------|----------|--------|-------|-------|
| Debuff & CC effect tracking | Yes (real-time) | Partial | Incomplete | #828 | Track applied CC effects on group members |
| Spell twist & medley support | Yes (class-specific) | No | Missing | #831 | Support for twist/medley casting for bards |
| Resurrection automation | Yes (combat recovery) | No | Missing | #833 | Automatic resurrection handling post-wipe |
| AA spend automation | Yes (point management) | No | Missing | #834 | Auto-spend alternate ability points |
| Advanced spell optimization | Yes (rotation DB) | Partial | Incomplete | #852 | Enhanced spell database with casting optimization |

### Inventory & Loot

| Feature | OpenVanilla | TextQuest | Status | Issue | Notes |
|---------|-------------|----------|--------|-------|-------|
| Auto-loot sorting | Yes (intelligent rules) | No | Missing | #824 | Automated loot prioritization and sorting |
| Auto-banking | Yes (inventory sync) | No | Missing | #825 | Automatic banker interactions and storage |
| Dialog automation | Yes (quest/trade/rez) | Partial | Incomplete | #826 | Generalized dialog response handling |

### Multiboxing & Coordination

| Feature | OpenVanilla | TextQuest | Status | Issue | Notes |
|---------|-------------|----------|--------|-------|-------|
| Advanced group management | Yes (dynamic targeting) | Partial | Incomplete | #836 | Intelligent group member targeting and swapping |
| Raid coordination | Yes (multi-group tools) | No | Missing | #838 | Raid-wide pull coordination and management |
| Tell relaying & chat forwarding | Yes (cross-client chat) | No | Missing | #851 | Forward tells/broadcast messages across clients |

### Monitoring & Analytics

| Feature | OpenVanilla | TextQuest | Status | Issue | Notes |
|---------|-------------|----------|--------|-------|-------|
| Kill tracking & DPS metrics | Yes (real-time) | Partial | Incomplete | #840 | Enhanced kill tracking with DPS analytics |
| XP tracking & leveling analytics | Yes (session stats) | Partial | Incomplete | #856 | Extended XP gain analytics and projections |
| Platinum & economy tracking | Yes (item values) | Partial | Incomplete | #849 | Track plat flow and item economics |

### Automation & Utilities

| Feature | OpenVanilla | TextQuest | Status | Issue | Notes |
|---------|-------------|----------|--------|-------|-------|
| Auto-forage & resource gathering | Yes (automated) | No | Missing | #847 | Automated forage/picking loop |
| Collectible & tribute management | Yes (auto-turnin) | No | Missing | #848 | Automated collectible/tribute handling |
| Vendor automation | Yes (buying/selling) | Partial | Incomplete | #830 | Buy/sell logic and optimization |
| Skill leveling automation | Yes (training loop) | No | Missing | #854 | Automated tradeskill progression |
| Quest tracking & task automation | Yes (task system) | No | Missing | #855 | Consolidated quest/task management |

### Integration & System

| Feature | OpenVanilla | TextQuest | Status | Issue | Notes |
|---------|-------------|----------|--------|-------|-------|
| Discord webhook enhancements | Yes (advanced formatting) | Partial | Incomplete | #839 | Rich embeds, event streaming, live updates |
| Crash reporting & recovery | Yes (session restore) | No | Missing | #853 | Auto-recovery from crashes with state persistence |

---

## Implementation Status Summary

### Fully Implemented in TextQuest
- Memory reading and process monitoring
- DLL injection and IPC
- Navigation with meshes
- Combat automation for 17 classes
- Camp loop automation
- Login automation
- TUI dashboard with real-time updates
- Web dashboard
- Packet monitoring and capture
- Basic Discord webhooks
- Credentials storage and management
- Soul Engine (LLM-driven personalities)

### Partially Implemented (Incomplete)
- Metrics and tracking (missing advanced analytics)
- Vendor automation (basic buy/sell only)
- Dialog handling (basic quest/trade, lacks generalization)
- Group management (basic member targeting)
- Discord integration (webhooks only, lacks rich embeds)
- Spell optimization (basic rotation, lacks advanced DB)

### Missing from TextQuest (OpenVanilla has)
- Plugin/scripting system (Lua-based extensibility)
- EQBC inter-character protocol
- Advanced loot sorting with rule engines
- Auto-banking and inventory sync
- Equipment set management and swapping
- Focus effect optimization
- Skill training automation
- Collectible/tribute automation
- Advanced spawn tracking
- Event/trigger system
- HUD customization overlays
- Raid coordination protocols
- Kill tracking with DPS metrics
- Audio alert system
- Window positioning API
- Automated forage/gathering
- Quest task consolidation system
- Resurrection automation

---

## Gap Prioritization (Recommended)

### High Priority (User Experience Impact)
1. Plugin/scripting system (#819, #820) — enables community extensibility
2. Auto-loot sorting (#824) — critical for efficiency
3. Raid coordination (#838) — enables team play scaling
4. Advanced group management (#836) — improves multiboxing UX

### Medium Priority (Operational Quality)
5. Skill training automation (#854) — tradeskill farming
6. Auto-banking (#825) — inventory management
7. Tell relaying (#851) — cross-client communication
8. Advanced metrics (#840, #856) — analytics visibility

### Lower Priority (Nice-to-Have)
9. Forage automation (#847) — gathering loop
10. Collectible management (#848) — convenience
11. Crash recovery (#853) — reliability
12. Audio alerts — accessibility

---

## Research Sources

- OpenVanilla GitHub: https://github.com/RedGuides/openvanilla
- MacroQuest Documentation: https://docs.macroquest.org/
- RedGuides Community: https://www.redguides.com/
- MQ2 Plugin Repositories
- TextQuest implementation-roadmap.md
- Issue tracking: #819-#856

## Next Steps

1. Evaluate plugin system architecture before other feature additions
2. Prioritize issues by ROI for user experience
3. Assign estimated effort for each gap (T-shirt sizes)
4. Plan implementation roadmap (M9-M11+)
5. Consider which features are blocking other features
