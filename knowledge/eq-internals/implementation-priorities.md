# Implementation Priorities for XP Farm (2026-03-29)

From MQ2 deep research analysis.

## What Frostreaver Has vs MQ2

| Area | MQ2 | Frostreaver | Gap |
|------|-----|-------------|-----|
| Navigation | Navmesh + pathfinding | Waypoint-based | Navmesh format unknown (OK for TLP) |
| Movement | `/stick` command driven | Direct speed/heading write | Need CPhysicsInfo offset validation |
| Combat | `/melee` + assist | DoAttack() hook | Good coverage |
| Casting | CastSpell() wrapper | CastSpell() hook | Good coverage |
| Healing | MQ2NetHeal broadcast | IPC + HolyShit conditionals | Good foundation |
| Loot | Corpse click automation | Not implemented | Need UI automation |
| Camp | Waypoint nav + leash | Camp positioning + return | Good coverage |

## Priority Order

### Phase 1 (Critical for XP farm)
1. Validate CPhysicsInfo heading/speed writes for movement control
2. Camp management + leash detection (DONE)
3. Combat targeting + assist (DONE)

### Phase 2 (High)
4. Self-heal + mana management
5. Basic buff rotation
6. Multi-client health polling via IPC

### Phase 3 (Medium)
7. Basic loot automation (click corpse)
8. Spell interruption detection
9. Debuff application (slows)

### Phase 4 (Nice-to-have)
- Navmesh pathfinding
- Advanced pulling
- Economy/vendor automation

## Key Insight
"Frostreaver is not missing major systems — it's executing them better
than MQ2's hackish approach." The architecture is sound; the gaps are
in data validation (live offset testing) not in design.
