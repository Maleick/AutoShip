# TextQuest Keyboard Shortcuts Guide

## Overview

TextQuest supports both global hotkeys (available to all characters) and character-specific hotkeys. This guide documents all available keyboard shortcuts by category.

**Legend:**

- `Shift` + Key = Hold Shift and press key
- `Alt` + Key = Hold Alt and press key
- `Ctrl` + Key = Hold Ctrl and press key

---

## Global Hotkeys

Global hotkeys are available to all characters and can be used at any time during gameplay.

### Combat Controls

| Shortcut | Action    | Description              |
| -------- | --------- | ------------------------ |
| **E**    | Engage    | Start combat with target |
| **D**    | Disengage | Stop combat              |
| **L**    | Loot      | Loot corpses             |
| **R**    | Repeat    | Repeat last command      |

### Navigation & UI

| Shortcut | Action      | Description                                |
| -------- | ----------- | ------------------------------------------ |
| **?**    | Help        | Open help screen / show keybindings        |
| **F1**   | Help (alt)  | Alternate keybinding to open help          |
| **Tab**  | Cycle Panel | Switch focus between active panels         |
| **M**    | Map         | Toggle map visibility (in Overview screen) |

### Search & Tools

| Shortcut | Action | Description       |
| -------- | ------ | ----------------- |
| **/**    | Search | Search for spawns |

### Special Modes

| Shortcut | Action       | Description                                |
| -------- | ------------ | ------------------------------------------ |
| **P**    | Privacy Mode | Toggle privacy mode (hides sensitive info) |
| **F8**   | Alert Panel  | Toggle alert panel visibility              |

---

## Character-Specific Hotkeys

Character-specific hotkeys are unique to each class and provide access to class abilities and commands.

### Warrior

| Shortcut | Action         | Description                                       |
| -------- | -------------- | ------------------------------------------------- |
| **E**    | Warrior Stance | Take defensive stance _(overrides global engage)_ |

### Wizard

| Shortcut | Action      | Description                                      |
| -------- | ----------- | ------------------------------------------------ |
| **M**    | Mana Shield | Cast mana shield _(overrides global map toggle)_ |
| **F2**   | Evocation   | Cast evocation (favorite spell)                  |

### Cleric

| Shortcut | Action     | Description               |
| -------- | ---------- | ------------------------- |
| **H**    | Heal       | Cast heal on focus target |
| **G**    | Group Heal | Cast group heal           |

---

## Help Access

Press **?** or **F1** in-game to view this keybinding reference at any time.

---

## Tips & Troubleshooting

### Key Override Behavior

Some character-specific hotkeys override global hotkeys with the same key. For example:

- Warrior: **E** activates "Warrior Stance" instead of "Engage"
- Wizard: **M** casts "Mana Shield" instead of toggling the map

This allows classes to have class-appropriate primary actions while still maintaining access to other hotkeys.

### Discovering New Hotkeys

- Use **Tab** to cycle through UI panels and discover available options
- Check the help screen (**?**) whenever you're unsure of a command
- Your current hotkeys are tailored to your character class

### Customizing Hotkeys

Hotkeys are defined in `config/hotkeys.toml`. To customize:

1. Locate the hotkey entry in the configuration file
2. Modify the `binding` (key + modifiers) or `action` field
3. Restart the game to apply changes

---

## Accessibility Notes

- All shortcuts use single keys or function keys for accessibility
- Navigation can be done entirely via keyboard using **Tab** and directional keys
- Privacy mode (**P**) hides sensitive information from screen readers and overlays

---

## See Also

- [User Guide](./USER_GUIDE.md)
- [Controls Reference](./CONTROLS_REFERENCE.md)
- [TUI Dashboard](../textquest-tui/README.md)
