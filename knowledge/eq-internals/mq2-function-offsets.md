# MQ2 Function Offsets — Key Discoveries (2026-03-29)

Source: Deep research of mq2-reference codebase.

## Movement (CPhysicsInfo)
```
CPhysicsInfo struct:
  0x00: Y (float)
  0x04: X (float)
  0x08: Z (float)
  0x0c: SpeedY, 0x10: SpeedX, 0x14: SpeedZ
  0x18: SpeedRun
  0x1c: Heading
  0x20: Angle
```

Key functions:
- `MovePlayer()` — called every frame (PlayerClient.h:673)
- `FacePlayer(PlayerZoneClient*)` — turn to face target (690)
- `TurnOffAutoFollow()` — stop following (947)
- `CheckForJump()` — jump detection (667)

## Casting
- `CastSpell()` @ 0x1400D9F20
- `BardCastBard()` @ 0x1400D8390 (bard-specific)
- `CanUseMemorizedSpellSlot()` @ 0x1400D9E80
- `GetCastingTimeModifier()` @ 0x1400DE630
- `SpellDuration()` @ 0x1400E7490
- `Cur_Mana()` @ 0x1400EF6B0
- `Max_Mana()` @ 0x1402F6240
- `GetManaRegen()` @ 0x1400F84F0

## Combat
- `DoAttack(slot, skill, Target)` @ 0x14031B890
  - slot: 0=primary, 1=secondary, 2=ranged
- `AllowedToAttack()` — check if valid target
- `CanIHit()` — LOS/facing check

## Loot
- `pinstActiveCorpse_x` @ 0x140E8E390
- `__do_loot_x` @ 0x14022C0D0
- `CAdvancedLootWnd::DoAdvLootAction()` @ 0x1400B0840

## Global Pointers
- `pinstSpellManager_x` @ 0x140F0E6F0

## Aggro
```
AggroMeterManagerClient:
- aggroData[30]: per-party/raid/xtarget aggro values
- AggroTargetID: current combat target
- AggroSecondaryID: NPC's current target
```
