/**
 * Vi-style keybinding utilities for TUI navigation and commands.
 *
 * Supported Vi keybindings:
 * - hjkl: movement (left, down, up, right)
 * - w/b: word navigation (forward/back in search)
 * - /: search forward
 * - ?: search backward
 * - :: command entry
 * - u/ctrl-r: undo/redo
 */

export interface ViKeybind {
  key: string;
  ctrl?: boolean;
  shift?: boolean;
  action: string;
  description: string;
}

export const VI_KEYBINDINGS: ViKeybind[] = [
  // Movement (hjkl)
  {
    key: "h",
    action: "move-left",
    description: "Move left",
  },
  {
    key: "j",
    action: "move-down",
    description: "Move down",
  },
  {
    key: "k",
    action: "move-up",
    description: "Move up",
  },
  {
    key: "l",
    action: "move-right",
    description: "Move right",
  },

  // Word navigation
  {
    key: "w",
    action: "word-forward",
    description: "Move word forward",
  },
  {
    key: "b",
    action: "word-backward",
    description: "Move word backward",
  },

  // Search
  {
    key: "/",
    action: "search-forward",
    description: "Search forward",
  },
  {
    key: "?",
    shift: true,
    action: "search-backward",
    description: "Search backward",
  },

  // Command entry
  {
    key: ":",
    shift: true,
    action: "command-entry",
    description: "Enter command mode",
  },

  // Undo/Redo
  {
    key: "u",
    action: "undo",
    description: "Undo",
  },
  {
    key: "r",
    ctrl: true,
    action: "redo",
    description: "Redo",
  },
];

export function getViActionForKeyEvent(
  key: string,
  ctrl: boolean,
  shift: boolean,
  alt: boolean,
): string | null {
  // Don't process Vi keybindings if Alt is pressed (reserved for system shortcuts)
  if (alt) {
    return null;
  }

  const binding = VI_KEYBINDINGS.find((kb) => {
    const keyMatch = kb.key.toLowerCase() === key.toLowerCase();
    const ctrlMatch = (kb.ctrl || false) === ctrl;
    const shiftMatch = (kb.shift || false) === shift;
    return keyMatch && ctrlMatch && shiftMatch;
  });

  return binding?.action ?? null;
}

/**
 * Map Vi action to handler function.
 * Returns true if the action was handled, false otherwise.
 */
export function handleViAction(
  action: string,
  handlers: {
    onMoveLeft?: () => void;
    onMoveRight?: () => void;
    onMoveUp?: () => void;
    onMoveDown?: () => void;
    onWordForward?: () => void;
    onWordBackward?: () => void;
    onSearchForward?: () => void;
    onSearchBackward?: () => void;
    onCommandEntry?: () => void;
    onUndo?: () => void;
    onRedo?: () => void;
  },
): boolean {
  switch (action) {
    case "move-left":
      handlers.onMoveLeft?.();
      return true;
    case "move-right":
      handlers.onMoveRight?.();
      return true;
    case "move-up":
      handlers.onMoveUp?.();
      return true;
    case "move-down":
      handlers.onMoveDown?.();
      return true;
    case "word-forward":
      handlers.onWordForward?.();
      return true;
    case "word-backward":
      handlers.onWordBackward?.();
      return true;
    case "search-forward":
      handlers.onSearchForward?.();
      return true;
    case "search-backward":
      handlers.onSearchBackward?.();
      return true;
    case "command-entry":
      handlers.onCommandEntry?.();
      return true;
    case "undo":
      handlers.onUndo?.();
      return true;
    case "redo":
      handlers.onRedo?.();
      return true;
    default:
      return false;
  }
}
