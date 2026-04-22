# Pet and Charm Management

TextQuest automates two overlapping but distinct systems: **summoned pets** (Necromancer, Magician, Shaman, Beastlord, Shadow Knight) and **charmed mobs** (Enchanter). Both surface through the Extended Target List's `MyPet` / `MyPetTarget` slots and share the same `PetStatus` snapshot type inside the DLL combat FSM.

---

## How the DLL Tracks Pets

### PetStatus snapshot

On every combat tick the FSM builds a `PetStatus` from the Extended Target List:

```rust
pub struct PetStatus {
    pub spawn_id: Option<u32>,   // None = no active pet
    pub target_id: Option<u32>,  // None = pet is idle
}
```

`spawn_id` comes from the `XTargetType::MyPet` slot; `target_id` from `XTargetType::MyPetTarget`. Helper methods:

| Method | Returns |
|--------|---------|
| `has_pet()` | `true` when any spawn occupies `MyPet` |
| `is_attacking(id)` | `true` when `MyPetTarget` matches `id` |

### Pet commands issued by the FSM

The `execute_pet_action` method dispatches slash commands based on the `PetAction` variant returned by each class strategy:

| Variant | Command(s) sent |
|---------|----------------|
| `PetAction::Attack` | `/pet attack` + `/pet focus` |
| `PetAction::Buff { spell }` | Target pet by ID → cast spell → restore original target |

Classes that own a pet (SK, Shaman, Necro, Mage, Beastlord) are listed in `PET_CLASSES` in `state.rs`. On engage the FSM calls `/pet attack` + `/pet focus`; on disengage it calls `/pet back`.

---

## Summoned Pet Classes

### Magician (class 13)

The Magician strategy (`textquest-dll/src/combat/classes/magician.rs`) adds full pet lifecycle management:

1. **Engagement** — sends `/pet attack` + `/pet focus` on engage via the default `PET_CLASSES` path.
2. **Pet buff** — after summoning a new pet (detected by spawn ID change), buffs it once with the strongest available Burnout spell, then marks the pet ID as buffed so the buff is not recast unless a new pet appears.
3. **Summon** — if mana allows and no pet is active, casts the best Summon Companion / Monster Summoning line.

Relevant ability set names: `PetBuff`, `PetSummon`, `UtilitySummon`.

### Necromancer (class 11) / Shadow Knight (class 5)

Both classes send the pet to attack on engage and recall it on disengage. Necromancer additionally uses the `pet_action` hook to manage DoT refreshes and undead-focus spell timing.

### Shaman (class 10) / Beastlord (class 15)

Pet attack follows the same PET_CLASSES path. Neither class overrides `pet_action` beyond the default.

---

## Enchanter Charm System

### Overview

The Enchanter (class 14) does not summon a pet; it **charms** a hostile mob and redirects it as a temporary ally. Charm is brittle — it breaks on resist, damage, and duration expiry — so the combat FSM treats recharm as the highest-priority CC action.

### Charm spell line

Configured in `EnchanterStrategy::build_ability_sets()` under the `"Charm"` ability set:

| Level | Spell |
|-------|-------|
| 12 | Charm |
| 30 | Beguile |
| 46 | Cajoling Whispers |
| 51 | Allure |
| 58 | Boltran's Agacerie |
| 65 | Command of Druzzil |

`resolve_abilities()` automatically selects the strongest spell the character can cast at their current level.

### Charm break detection

A charmed mob occupies the `XTargetType::MyPet` Extended Target slot. When charm breaks the mob leaves that slot and reappears in the nearby-enemies list as a hostile. The FSM detects this state change as:

```
pet_status.has_pet() == false  AND  hostile mob present in nearby_enemies
```

`EnchanterStrategy::first_uncontrolled_add()` locates the first off-target enemy that is not mezzed or already charmed, making it available to `select_target()` for immediate recharm or mez.

### Recharm cycle

1. Charm breaks → mob vanishes from `MyPet` slot.
2. `first_uncontrolled_add()` returns the mob's spawn ID.
3. `select_target()` queues it as the CC target.
4. Rotation engine fires `Tash` → `Mez` or directly recasts `Charm` depending on rotation group priority and mana constraints.
5. On successful charm the mob reappears in `MyPet` and `pet_status.has_pet()` returns `true`.

### Pet commands on a charmed mob

Because the charmed mob occupies `MyPet`, the generic `pet_attack_action` helper applies:

- If the charmed mob is **idle** (no `MyPetTarget` slot), `pet_attack_action` returns `PetAction::Attack`.
- If the charmed mob is **already attacking** the current target, `pet_attack_action` returns `None` to avoid redundant commands.

---

## Configuration

### Enchanter config profile

The Enchanter reads `data/class-configs/enchanter.toml`. Relevant sections:

```toml
[[cc_abilities]]
cc_type = "charm"
name    = "Boltran's Agacerie"   # overridden per level-range block
```

Each `[[level_range]]` block specifies the charm spell name for that level bracket. The runtime resolution from `resolve_abilities()` and the TOML profile are validated to match in `enchanter_toml_breakpoints_match_runtime_resolution` (test in `enchanter.rs`).

### Pet class config

Pet-class behavior is currently hard-coded in `PET_CLASSES` in `state.rs`. No TOML override is needed for the engage/disengage commands; only `pet_action` overrides (e.g., Magician's `PetBuff`) are class-strategy logic.

---

## Troubleshooting

| Symptom | Likely cause | Resolution |
|---------|-------------|------------|
| Charmed mob not attacking | `MyPet` slot not yet populated after cast | Wait one additional tick; EQ updates xtarget list asynchronously |
| Charm breaks immediately on engage | Target is immune or too high level | Check `resolve_abilities` resolved spell level; verify target is charmable |
| Pet not attacking after summon | `PetSummon` cast succeeded but `MyPet` slot empty | Verify offset DB has correct `MyPet` slot index for current EQ version |
| Magician pet buff re-cast on every engage | `buffed_pet_id` cleared on abilities re-resolve | Expected — buff re-fires once per new pet, then stops |
| Recharm not firing after charm break | Mana below rotation threshold (35%) | Raise mana before engaging; consider a lower `mana_above` threshold in config |

---

## Related

- `textquest-dll/src/combat/strategy.rs` — `PetStatus`, `PetAction`, `pet_attack_action`, `CombatContext::pet_status()`
- `textquest-dll/src/combat/classes/enchanter.rs` — `EnchanterStrategy`, `build_ability_sets()`, `select_target()`, `first_uncontrolled_add()`
- `textquest-dll/src/combat/classes/magician.rs` — `MagicianStrategy`, `PendingCast::PetBuff`, `pet_action()`
- `textquest-dll/src/combat/state.rs` — `PET_CLASSES`, `execute_pet_action()`, `on_engage`, `on_disengage`
- `textquest-common/src/types.rs` — `PetData` (IPC pet state sent to orchestrator)
- `textquest-common/src/combat.rs` — `XTargetType::MyPet`, `XTargetType::MyPetTarget`, `ExtendedTargetList`
- `docs/superpowers/plans/2026-04-15-charm-break-detection-auto-recharm.md` — implementation plan for FSM-level charm tracking
