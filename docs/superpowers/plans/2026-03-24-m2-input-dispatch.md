# M2: Input Dispatch + Multi-Client Manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Send keystrokes and EQ commands to specific eqgame.exe windows from an external Rust process, enabling multibox control of up to 36 clients across multiple machines.

**Architecture:** External input dispatch via `PostMessage` (WM_KEYDOWN/WM_KEYUP) to targeted EQ window handles. A multi-client manager tracks all running eqgame.exe processes, maps each to a character name by reading memory, and routes commands to the correct window. A command parser translates high-level commands (`/cast 1`, `/target "Lady Vox"`) into sequences of keypress messages. All Windows API calls are confined behind `#[cfg(windows)]` with non-Windows stubs for cross-compilation.

**Tech Stack:** Rust, `windows` crate (Win32 PostMessage/SendInput/HWND APIs), existing ProcessHandle/SpawnInfo from M1, TOML config for keybind mappings.

**Key Design Decision — PostMessage vs SendInput:**
MQ2 uses DirectInput detours (DLL injection) which we can't do. Our options:

- **PostMessage** — sends WM_KEYDOWN/WM_KEYUP to a specific HWND without requiring focus. Works for most EQ text input and keybind-triggered actions. Does NOT work for mouse input or DirectInput-only bindings.
- **SendInput** — simulates real keyboard events but requires the window to have focus. We'd need to cycle focus across windows rapidly.
- **Hybrid approach (recommended)** — PostMessage for most keystroke dispatch (no focus required = parallel control), SendInput as fallback for edge cases that need real input events.

---

## File Structure

```
src/
  input/
    mod.rs              # Module exports
    keys.rs             # Virtual key code mappings (VK_* constants, EQ keybind map)
    dispatch.rs         # PostMessage/SendInput wrappers — send keystrokes to HWND
    sequence.rs         # KeySequence type — ordered list of key events with delays
  client/
    mod.rs              # Module exports
    manager.rs          # ClientManager — tracks all EQ processes, maps PID→character
    session.rs          # EqSession — single EQ client (PID, HWND, ProcessHandle, character name)
  command/
    mod.rs              # Module exports
    parser.rs           # Parse "/cast 1", "/target Mob", "/key F1" into Command enum
    executor.rs         # Execute Command by resolving it to KeySequences on the right session
    types.rs            # Command enum, CommandResult, EqKeybind mapping
  process/
    window.rs           # (MODIFY) — add find_eq_windows(), focus management
    memory.rs           # (existing, no changes)
  tui/
    app.rs              # (MODIFY) — add ClientManager, multi-client display state
    ui.rs               # (MODIFY) — add client list panel, command input bar
    event.rs            # (MODIFY) — add command input mode keybindings
  config.rs             # (MODIFY) — add keybind_profile, input settings
```

### File Responsibilities

| File                  | Responsibility                                                                             |
| --------------------- | ------------------------------------------------------------------------------------------ |
| `input/keys.rs`       | VK\_\* constant definitions, scancode mappings, `EqKey` enum covering all EQ-relevant keys |
| `input/dispatch.rs`   | `send_key(hwnd, key, modifiers)`, `send_string(hwnd, text)` — raw PostMessage wrappers     |
| `input/sequence.rs`   | `KeySequence` — ordered list of `(EqKey, Duration)` pairs, `execute(hwnd)` method          |
| `client/session.rs`   | `EqSession` — one EQ client: PID, HWND, ProcessHandle, character name, group assignment    |
| `client/manager.rs`   | `ClientManager` — Vec of EqSessions, discovery, routing commands by character name         |
| `command/types.rs`    | `Command` enum variants: Cast, Target, Key, Assist, Follow, Say, Raw                       |
| `command/parser.rs`   | `parse(input: &str) -> Result<Command>` — slash command parser                             |
| `command/executor.rs` | `execute(cmd, session, keybinds) -> Result<()>` — resolve command to key sequences         |

---

## Task 1: Virtual Key Mappings (`input/keys.rs`)

**Files:**

- Create: `src/input/mod.rs`
- Create: `src/input/keys.rs`

- [ ] **Step 1: Create input module with key definitions**

```rust
// src/input/mod.rs
pub mod keys;

// src/input/keys.rs
/// Virtual key codes for EQ-relevant keys.
/// These match Windows VK_* constants from WinUser.h.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum EqKey {
    // Letters
    A = 0x41, B = 0x42, C = 0x43, D = 0x44, E = 0x45,
    F = 0x46, G = 0x47, H = 0x48, I = 0x49, J = 0x4A,
    K = 0x4B, L = 0x4C, M = 0x4D, N = 0x4E, O = 0x4F,
    P = 0x50, Q = 0x51, R = 0x52, S = 0x53, T = 0x54,
    U = 0x55, V = 0x56, W = 0x57, X = 0x58, Y = 0x59,
    Z = 0x5A,

    // Numbers
    Num0 = 0x30, Num1 = 0x31, Num2 = 0x32, Num3 = 0x33,
    Num4 = 0x34, Num5 = 0x35, Num6 = 0x36, Num7 = 0x37,
    Num8 = 0x38, Num9 = 0x39,

    // Function keys
    F1 = 0x70, F2 = 0x71, F3 = 0x72, F4 = 0x73,
    F5 = 0x74, F6 = 0x75, F7 = 0x76, F8 = 0x77,
    F9 = 0x78, F10 = 0x79, F11 = 0x7A, F12 = 0x7B,

    // Navigation
    Enter = 0x0D, Escape = 0x1B, Tab = 0x09,
    Space = 0x20, Backspace = 0x08,

    // Arrow keys
    Up = 0x26, Down = 0x28, Left = 0x25, Right = 0x27,

    // Modifiers (for lParam construction)
    Shift = 0x10, Control = 0x11, Alt = 0x12,

    // Numpad
    Numpad0 = 0x60, Numpad1 = 0x61, Numpad2 = 0x62,
    Numpad3 = 0x63, Numpad4 = 0x64, Numpad5 = 0x65,
    Numpad6 = 0x66, Numpad7 = 0x67, Numpad8 = 0x68,
    Numpad9 = 0x69,

    // Punctuation
    Slash = 0xBF,     // '/' key — opens EQ command line
    Semicolon = 0xBA, // ';'
    Comma = 0xBC,
    Period = 0xBE,
    Minus = 0xBD,
    Equals = 0xBB,
    Tilde = 0xC0,     // '`' / '~'
}

/// Modifier key flags for key combinations.
#[derive(Debug, Clone, Copy, Default)]
pub struct EqKeyModifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// A single key event — press or release.
#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    pub key: EqKey,
    pub modifiers: EqKeyModifiers,
    pub down: bool,
}

impl EqKey {
    /// Get the Windows scancode for this virtual key.
    /// Used to construct the lParam for WM_KEYDOWN/WM_KEYUP.
    pub fn scancode(self) -> u8 {
        // MapVirtualKey equivalent — hardcoded for common keys
        match self {
            EqKey::Enter => 0x1C,
            EqKey::Escape => 0x01,
            EqKey::Tab => 0x0F,
            EqKey::Space => 0x39,
            EqKey::Slash => 0x35,
            // For letters, the scancode is key-dependent but we can
            // use MapVirtualKeyW at runtime on Windows
            _ => 0,
        }
    }

    /// Parse a key name string (e.g., "F1", "a", "Enter") into an EqKey.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "a" => Some(EqKey::A), "b" => Some(EqKey::B), "c" => Some(EqKey::C),
            "d" => Some(EqKey::D), "e" => Some(EqKey::E), "f" => Some(EqKey::F),
            "g" => Some(EqKey::G), "h" => Some(EqKey::H), "i" => Some(EqKey::I),
            "j" => Some(EqKey::J), "k" => Some(EqKey::K), "l" => Some(EqKey::L),
            "m" => Some(EqKey::M), "n" => Some(EqKey::N), "o" => Some(EqKey::O),
            "p" => Some(EqKey::P), "q" => Some(EqKey::Q), "r" => Some(EqKey::R),
            "s" => Some(EqKey::S), "t" => Some(EqKey::T), "u" => Some(EqKey::U),
            "v" => Some(EqKey::V), "w" => Some(EqKey::W), "x" => Some(EqKey::X),
            "y" => Some(EqKey::Y), "z" => Some(EqKey::Z),
            "enter" | "return" => Some(EqKey::Enter),
            "esc" | "escape" => Some(EqKey::Escape),
            "tab" => Some(EqKey::Tab),
            "space" => Some(EqKey::Space),
            "f1" => Some(EqKey::F1), "f2" => Some(EqKey::F2),
            "f3" => Some(EqKey::F3), "f4" => Some(EqKey::F4),
            "f5" => Some(EqKey::F5), "f6" => Some(EqKey::F6),
            "f7" => Some(EqKey::F7), "f8" => Some(EqKey::F8),
            "f9" => Some(EqKey::F9), "f10" => Some(EqKey::F10),
            "f11" => Some(EqKey::F11), "f12" => Some(EqKey::F12),
            "/" | "slash" => Some(EqKey::Slash),
            _ => None,
        }
    }
}
```

- [ ] **Step 2: Add `mod input;` to main.rs**

Add `mod input;` after existing module declarations in `src/main.rs`.

- [ ] **Step 3: Verify build**

Run: `cargo build 2>&1`
Expected: Clean compile, no errors.

- [ ] **Step 4: Commit**

```bash
git add src/input/
git commit -m "feat(input): add virtual key code mappings and EqKey enum"
```

---

## Task 2: PostMessage Keystroke Dispatch (`input/dispatch.rs`)

**Files:**

- Create: `src/input/dispatch.rs`
- Modify: `src/input/mod.rs` — add `pub mod dispatch;`

- [ ] **Step 1: Write dispatch module with PostMessage wrappers**

```rust
// src/input/dispatch.rs
use anyhow::{Result, bail};
use std::time::Duration;
use std::thread;

use super::keys::{EqKey, EqKeyModifiers, KeyEvent};

/// Opaque window handle for cross-platform compilation.
#[derive(Debug, Clone, Copy)]
pub struct WindowTarget {
    #[cfg(windows)]
    pub hwnd: windows::Win32::Foundation::HWND,
    #[cfg(not(windows))]
    pub hwnd: u64, // stub
}

/// Default delay between key down and key up events.
const KEY_PRESS_DURATION: Duration = Duration::from_millis(50);

/// Default delay between sequential keystrokes (e.g., typing a string).
const INTER_KEY_DELAY: Duration = Duration::from_millis(30);

/// Send a single key press (down + delay + up) to the target window.
pub fn send_key(target: WindowTarget, key: EqKey, modifiers: EqKeyModifiers) -> Result<()> {
    send_key_down(target, key, modifiers)?;
    thread::sleep(KEY_PRESS_DURATION);
    send_key_up(target, key, modifiers)?;
    Ok(())
}

/// Send WM_KEYDOWN to target window via PostMessage.
#[cfg(windows)]
pub fn send_key_down(target: WindowTarget, key: EqKey, modifiers: EqKeyModifiers) -> Result<()> {
    use windows::Win32::UI::WindowsAndMessaging::{
        PostMessageW, WM_KEYDOWN, WM_SYSKEYDOWN,
    };
    use windows::Win32::Foundation::{WPARAM, LPARAM};

    // Send modifier keys first if needed
    if modifiers.shift {
        post_key_msg(target.hwnd, WM_KEYDOWN, EqKey::Shift, false)?;
    }
    if modifiers.ctrl {
        post_key_msg(target.hwnd, WM_KEYDOWN, EqKey::Control, false)?;
    }

    let msg = if modifiers.alt { WM_SYSKEYDOWN } else { WM_KEYDOWN };
    post_key_msg(target.hwnd, msg, key, modifiers.alt)?;

    Ok(())
}

/// Send WM_KEYUP to target window via PostMessage.
#[cfg(windows)]
pub fn send_key_up(target: WindowTarget, key: EqKey, modifiers: EqKeyModifiers) -> Result<()> {
    use windows::Win32::UI::WindowsAndMessaging::{
        PostMessageW, WM_KEYUP, WM_SYSKEYUP,
    };

    let msg = if modifiers.alt { WM_SYSKEYUP } else { WM_KEYUP };
    post_key_msg(target.hwnd, msg, key, modifiers.alt)?;

    // Release modifiers in reverse order
    if modifiers.ctrl {
        post_key_msg(target.hwnd, WM_KEYUP, EqKey::Control, false)?;
    }
    if modifiers.shift {
        post_key_msg(target.hwnd, WM_KEYUP, EqKey::Shift, false)?;
    }

    Ok(())
}

/// Construct lParam and post the message.
#[cfg(windows)]
fn post_key_msg(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    key: EqKey,
    alt_held: bool,
) -> Result<()> {
    use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
    use windows::Win32::Foundation::{WPARAM, LPARAM};

    let vk = key as u16;
    let scancode = key.scancode() as u32;
    let is_up = msg == 0x0101 || msg == 0x0105; // WM_KEYUP or WM_SYSKEYUP

    // lParam encoding:
    // bits 0-15:  repeat count (1)
    // bits 16-23: scancode
    // bit 24:     extended key flag
    // bit 29:     context code (1 if ALT held)
    // bit 30:     previous key state (1 if up)
    // bit 31:     transition state (1 if up)
    let mut lparam: u32 = 1; // repeat count = 1
    lparam |= scancode << 16;
    if alt_held { lparam |= 1 << 29; }
    if is_up { lparam |= (1 << 30) | (1 << 31); }

    unsafe {
        PostMessageW(hwnd, msg, WPARAM(vk as usize), LPARAM(lparam as isize))
    }.map_err(|e| anyhow::anyhow!("PostMessage failed: {}", e))?;

    Ok(())
}

/// Type a string into the target window (character by character).
/// Used for EQ chat/command input after pressing '/' or Enter.
pub fn send_string(target: WindowTarget, text: &str) -> Result<()> {
    for ch in text.chars() {
        if let Some(key) = char_to_key(ch) {
            let modifiers = EqKeyModifiers {
                shift: ch.is_ascii_uppercase() || "!@#$%^&*()_+{}|:\"<>?~".contains(ch),
                ..Default::default()
            };
            send_key(target, key, modifiers)?;
            thread::sleep(INTER_KEY_DELAY);
        }
    }
    Ok(())
}

/// Send an EQ slash command by typing "/" + command text + Enter.
pub fn send_eq_command(target: WindowTarget, command: &str) -> Result<()> {
    // Press '/' to open EQ command line
    send_key(target, EqKey::Slash, EqKeyModifiers::default())?;
    thread::sleep(Duration::from_millis(100));

    // Type the command (without leading '/')
    let cmd = command.strip_prefix('/').unwrap_or(command);
    send_string(target, cmd)?;

    // Press Enter to execute
    thread::sleep(Duration::from_millis(50));
    send_key(target, EqKey::Enter, EqKeyModifiers::default())?;

    Ok(())
}

fn char_to_key(ch: char) -> Option<EqKey> {
    match ch.to_ascii_lowercase() {
        'a' => Some(EqKey::A), 'b' => Some(EqKey::B), 'c' => Some(EqKey::C),
        'd' => Some(EqKey::D), 'e' => Some(EqKey::E), 'f' => Some(EqKey::F),
        'g' => Some(EqKey::G), 'h' => Some(EqKey::H), 'i' => Some(EqKey::I),
        'j' => Some(EqKey::J), 'k' => Some(EqKey::K), 'l' => Some(EqKey::L),
        'm' => Some(EqKey::M), 'n' => Some(EqKey::N), 'o' => Some(EqKey::O),
        'p' => Some(EqKey::P), 'q' => Some(EqKey::Q), 'r' => Some(EqKey::R),
        's' => Some(EqKey::S), 't' => Some(EqKey::T), 'u' => Some(EqKey::U),
        'v' => Some(EqKey::V), 'w' => Some(EqKey::W), 'x' => Some(EqKey::X),
        'y' => Some(EqKey::Y), 'z' => Some(EqKey::Z),
        '0' => Some(EqKey::Num0), '1' => Some(EqKey::Num1),
        '2' => Some(EqKey::Num2), '3' => Some(EqKey::Num3),
        '4' => Some(EqKey::Num4), '5' => Some(EqKey::Num5),
        '6' => Some(EqKey::Num6), '7' => Some(EqKey::Num7),
        '8' => Some(EqKey::Num8), '9' => Some(EqKey::Num9),
        ' ' => Some(EqKey::Space),
        '/' => Some(EqKey::Slash),
        _ => None,
    }
}

// Non-Windows stubs
#[cfg(not(windows))]
pub fn send_key_down(target: WindowTarget, key: EqKey, modifiers: EqKeyModifiers) -> Result<()> {
    tracing::debug!("STUB send_key_down: {:?} to {:?}", key, target.hwnd);
    Ok(())
}

#[cfg(not(windows))]
pub fn send_key_up(target: WindowTarget, key: EqKey, modifiers: EqKeyModifiers) -> Result<()> {
    tracing::debug!("STUB send_key_up: {:?} to {:?}", key, target.hwnd);
    Ok(())
}
```

- [ ] **Step 2: Add `pub mod dispatch;` to `src/input/mod.rs`**

- [ ] **Step 3: Verify build**

Run: `cargo build 2>&1`
Expected: Clean compile.

- [ ] **Step 4: Commit**

```bash
git add src/input/dispatch.rs src/input/mod.rs
git commit -m "feat(input): add PostMessage keystroke dispatch wrappers"
```

---

## Task 3: Key Sequence Type (`input/sequence.rs`)

**Files:**

- Create: `src/input/sequence.rs`
- Modify: `src/input/mod.rs` — add `pub mod sequence;`

- [ ] **Step 1: Create KeySequence — ordered key events with timing**

```rust
// src/input/sequence.rs
use std::time::Duration;
use anyhow::Result;

use super::keys::{EqKey, EqKeyModifiers};
use super::dispatch::{WindowTarget, send_key, send_eq_command};

/// A timed sequence of key actions to send to an EQ window.
#[derive(Debug, Clone)]
pub struct KeySequence {
    steps: Vec<KeyStep>,
}

#[derive(Debug, Clone)]
enum KeyStep {
    /// Press and release a single key with modifiers.
    Press { key: EqKey, modifiers: EqKeyModifiers },
    /// Type an EQ slash command (opens /, types text, presses Enter).
    EqCommand(String),
    /// Wait for a duration before the next step.
    Delay(Duration),
}

impl KeySequence {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    /// Number of steps in the sequence.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether the sequence is empty.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Add a keypress to the sequence.
    pub fn press(mut self, key: EqKey) -> Self {
        self.steps.push(KeyStep::Press { key, modifiers: EqKeyModifiers::default() });
        self
    }

    /// Add a modified keypress (e.g., Alt+F1).
    pub fn press_modified(mut self, key: EqKey, modifiers: EqKeyModifiers) -> Self {
        self.steps.push(KeyStep::Press { key, modifiers });
        self
    }

    /// Add an EQ slash command (e.g., "/target Lady Vox").
    pub fn eq_command(mut self, cmd: impl Into<String>) -> Self {
        self.steps.push(KeyStep::EqCommand(cmd.into()));
        self
    }

    /// Add a delay between steps.
    pub fn delay(mut self, duration: Duration) -> Self {
        self.steps.push(KeyStep::Delay(duration));
        self
    }

    /// Execute the full sequence against a target window.
    pub fn execute(&self, target: WindowTarget) -> Result<()> {
        for step in &self.steps {
            match step {
                KeyStep::Press { key, modifiers } => {
                    send_key(target, *key, *modifiers)?;
                }
                KeyStep::EqCommand(cmd) => {
                    send_eq_command(target, cmd)?;
                }
                KeyStep::Delay(d) => {
                    std::thread::sleep(*d);
                }
            }
        }
        Ok(())
    }
}
```

- [ ] **Step 2: Verify build**

Run: `cargo build 2>&1`

- [ ] **Step 3: Commit**

```bash
git add src/input/sequence.rs src/input/mod.rs
git commit -m "feat(input): add KeySequence for timed multi-step key dispatch"
```

---

## Task 4: EQ Session (`client/session.rs`)

**Files:**

- Create: `src/client/mod.rs`
- Create: `src/client/session.rs`

- [ ] **Step 1: Define EqSession — one EQ client instance**

```rust
// src/client/mod.rs
pub mod session;

// src/client/session.rs
use anyhow::Result;
use crate::input::dispatch::WindowTarget;
use crate::input::keys::EqKey;
use crate::input::sequence::KeySequence;
use crate::eq::structs::SpawnInfo;

/// Represents a single running EQ client with everything needed to interact with it.
#[derive(Debug)]
pub struct EqSession {
    /// Process ID of this eqgame.exe instance.
    pub pid: u32,
    /// Window handle for sending input.
    pub window: WindowTarget,
    /// Base address of eqgame.exe in this process's memory.
    pub eq_base: u64,
    /// Character name (read from memory on discovery).
    pub character_name: String,
    /// Which group this character belongs to (from config).
    pub group_id: Option<u32>,
    /// Last known player state (updated on refresh).
    pub player_info: Option<SpawnInfo>,
    /// Whether this session is responsive (last memory read succeeded).
    pub alive: bool,
}

impl EqSession {
    pub fn new(pid: u32, window: WindowTarget, eq_base: u64, character_name: String) -> Self {
        Self {
            pid,
            window,
            eq_base,
            character_name,
            group_id: None,
            player_info: None,
            alive: true,
        }
    }

    /// Send a key sequence to this EQ client.
    pub fn send_sequence(&self, seq: &KeySequence) -> Result<()> {
        seq.execute(self.window)
    }

    /// Send a raw EQ slash command to this client.
    pub fn send_command(&self, cmd: &str) -> Result<()> {
        crate::input::dispatch::send_eq_command(self.window, cmd)
    }

    /// Send a single keypress to this client.
    pub fn send_key(&self, key: EqKey) -> Result<()> {
        crate::input::dispatch::send_key(
            self.window, key, crate::input::keys::EqKeyModifiers::default()
        )
    }
}
```

- [ ] **Step 2: Add `mod client;` to main.rs**

- [ ] **Step 3: Verify build**

Run: `cargo build 2>&1`

- [ ] **Step 4: Commit**

```bash
git add src/client/
git commit -m "feat(client): add EqSession for single EQ client management"
```

---

## Task 5: Client Manager (`client/manager.rs`)

**Files:**

- Create: `src/client/manager.rs`
- Modify: `src/client/mod.rs` — add `pub mod manager;`
- Modify: `src/process/window.rs` — add `find_eq_windows()` function
- Modify: `src/process/memory.rs` — move `get_module_base` from `main.rs` to here as `pub fn`
- Modify: `src/main.rs` — call `process::memory::get_module_base()` instead of local fn

- [ ] **Step 0: Move `get_module_base` from `main.rs` to `process/memory.rs`**

The `get_module_base` function is currently a private function in `main.rs`. Move it to `src/process/memory.rs` as a public function so `client/manager.rs` can access it via `crate::process::memory::get_module_base`. Update `main.rs` to call `process::memory::get_module_base(pid)` instead.

- [ ] **Step 1: Add `find_eq_windows()` to process/window.rs**

Add a function that finds all windows with class name `_EverQuestwndclass` (the EQ window class, as seen in MQ2 source). This extends the existing `find_windows_by_title()` with a class-name-based search.

```rust
/// Find all EQ game windows by their window class name.
#[cfg(windows)]
pub fn find_eq_windows() -> Result<Vec<WindowHandle>> {
    find_windows_by_title("EverQuest")  // fallback: match title containing "EverQuest"
}

#[cfg(not(windows))]
pub fn find_eq_windows() -> Result<Vec<WindowHandle>> {
    Ok(Vec::new())
}
```

- [ ] **Step 2: Write ClientManager — discovers and tracks all EQ sessions**

```rust
// src/client/manager.rs
use anyhow::Result;
use std::collections::HashMap;

use super::session::EqSession;
use crate::input::dispatch::WindowTarget;
use crate::process::memory::{ProcessHandle, find_processes_by_name};
use crate::process::window::find_eq_windows;
use crate::eq::spawn;

/// Manages all known EQ client sessions.
pub struct ClientManager {
    /// All discovered sessions, keyed by PID.
    sessions: HashMap<u32, EqSession>,
}

impl ClientManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Discover running EQ processes and create sessions for new ones.
    pub fn discover(&mut self) -> Result<usize> {
        let pids = find_processes_by_name("eqgame.exe")?;
        let mut new_count = 0;

        for pid in pids {
            if self.sessions.contains_key(&pid) {
                continue;
            }

            match self.create_session(pid) {
                Ok(session) => {
                    tracing::info!(
                        pid,
                        name = %session.character_name,
                        "Discovered EQ client"
                    );
                    self.sessions.insert(pid, session);
                    new_count += 1;
                }
                Err(e) => {
                    tracing::warn!(pid, "Failed to attach to EQ process: {:#}", e);
                }
            }
        }

        Ok(new_count)
    }

    /// Create a new session for a given PID.
    fn create_session(&self, pid: u32) -> Result<EqSession> {
        let proc = ProcessHandle::open(pid)?;
        let eq_base = crate::process::memory::get_module_base(&proc)?;

        // Read character name from memory
        let character_name = match spawn::read_local_player(&proc, eq_base) {
            Ok(player) => player.displayed_name,
            Err(_) => format!("Unknown (PID {})", pid),
        };

        // Find the window handle for this PID
        let windows = find_eq_windows()?;
        let window = windows.iter()
            .find(|w| w.pid == pid)
            .map(|w| WindowTarget {
                #[cfg(windows)]
                hwnd: w.hwnd,
                #[cfg(not(windows))]
                hwnd: 0,
            })
            .unwrap_or(WindowTarget {
                #[cfg(windows)]
                hwnd: windows::Win32::Foundation::HWND::default(),
                #[cfg(not(windows))]
                hwnd: 0,
            });

        Ok(EqSession::new(pid, window, eq_base, character_name))
    }

    /// Get a session by character name (case-insensitive).
    pub fn by_name(&self, name: &str) -> Option<&EqSession> {
        let name_lower = name.to_lowercase();
        self.sessions.values()
            .find(|s| s.character_name.to_lowercase() == name_lower)
    }

    /// Get a session by PID.
    pub fn by_pid(&self, pid: u32) -> Option<&EqSession> {
        self.sessions.get(&pid)
    }

    /// Get all sessions.
    pub fn all_sessions(&self) -> Vec<&EqSession> {
        self.sessions.values().collect()
    }

    /// Get all sessions in a specific group.
    pub fn by_group(&self, group_id: u32) -> Vec<&EqSession> {
        self.sessions.values()
            .filter(|s| s.group_id == Some(group_id))
            .collect()
    }

    /// Send a command to a specific character by name.
    pub fn send_to(&self, character: &str, command: &str) -> Result<()> {
        let session = self.by_name(character)
            .ok_or_else(|| anyhow::anyhow!("No session found for '{}'", character))?;
        session.send_command(command)
    }

    /// Send a command to all sessions in a group.
    pub fn send_to_group(&self, group_id: u32, command: &str) -> Result<()> {
        for session in self.by_group(group_id) {
            if let Err(e) = session.send_command(command) {
                tracing::warn!(
                    name = %session.character_name,
                    "Failed to send command: {:#}", e
                );
            }
        }
        Ok(())
    }

    /// Send a command to ALL sessions.
    pub fn broadcast(&self, command: &str) -> Result<()> {
        for session in self.sessions.values() {
            if let Err(e) = session.send_command(command) {
                tracing::warn!(
                    name = %session.character_name,
                    "Failed to broadcast: {:#}", e
                );
            }
        }
        Ok(())
    }

    /// Remove dead sessions (processes that no longer exist).
    pub fn prune_dead(&mut self) {
        self.sessions.retain(|pid, session| {
            if !session.alive {
                tracing::info!(pid, name = %session.character_name, "Removing dead session");
                false
            } else {
                true
            }
        });
    }

    pub fn count(&self) -> usize {
        self.sessions.len()
    }
}
```

- [ ] **Step 3: Add `pub mod manager;` to client/mod.rs**

- [ ] **Step 4: Verify build**

Run: `cargo build 2>&1`

- [ ] **Step 5: Commit**

```bash
git add src/client/ src/process/window.rs
git commit -m "feat(client): add ClientManager for multi-client discovery and routing"
```

---

## Task 6: Command Types (`command/types.rs`)

**Files:**

- Create: `src/command/mod.rs`
- Create: `src/command/types.rs`

- [ ] **Step 1: Define the Command enum**

```rust
// src/command/mod.rs
pub mod types;

// src/command/types.rs
use crate::input::keys::EqKey;

/// A parsed command to execute on one or more EQ clients.
#[derive(Debug, Clone)]
pub enum Command {
    /// Send a raw EQ slash command: /say, /shout, /tell, etc.
    EqSlash(String),

    /// Cast a spell from a gem slot (1-12).
    Cast { gem: u8 },

    /// Target a spawn by name.
    Target { name: String },

    /// Press a specific key (with optional modifiers).
    Key { key: EqKey, shift: bool, ctrl: bool, alt: bool },

    /// Assist a player (target their target).
    Assist { name: String },

    /// Follow a player.
    Follow { name: String },

    /// Sit or stand.
    Sit,
    Stand,

    /// Attack on/off.
    Attack(bool),
}

/// Specifies which EQ clients should receive a command.
#[derive(Debug, Clone)]
pub enum CommandTarget {
    /// Send to a specific character by name.
    Character(String),
    /// Send to all characters in a group.
    Group(u32),
    /// Send to all connected characters.
    All,
    /// Send to the currently focused/selected character in the TUI.
    Current,
}

/// A command with its target routing.
#[derive(Debug, Clone)]
pub struct RoutedCommand {
    pub command: Command,
    pub target: CommandTarget,
}
```

- [ ] **Step 2: Add `mod command;` to main.rs**

- [ ] **Step 3: Verify build**

Run: `cargo build 2>&1`

- [ ] **Step 4: Commit**

```bash
git add src/command/
git commit -m "feat(command): add Command enum and routing types"
```

---

## Task 7: Command Parser (`command/parser.rs`)

**Files:**

- Create: `src/command/parser.rs`
- Modify: `src/command/mod.rs` — add `pub mod parser;`

- [ ] **Step 1: Implement command parser**

```rust
// src/command/parser.rs
use anyhow::{Result, bail};

use super::types::{Command, CommandTarget, RoutedCommand};
use crate::input::keys::EqKey;

/// Parse a command string into a RoutedCommand.
///
/// Syntax:
///   @<target> <command>     — route to specific character/group
///   @all <command>          — broadcast to all
///   @group:<n> <command>    — route to group N
///   <command>               — route to current selection
///
/// Commands:
///   /cast <gem>             — cast spell from gem slot
///   /target <name>          — target a spawn
///   /assist <name>          — assist a player
///   /follow <name>          — follow a player
///   /key <keyname>          — press a key
///   /sit, /stand            — sit/stand
///   /attack on|off          — toggle attack
///   /<anything else>        — raw EQ command passthrough
pub fn parse(input: &str) -> Result<RoutedCommand> {
    let input = input.trim();
    if input.is_empty() {
        bail!("Empty command");
    }

    // Parse target prefix
    let (target, cmd_str) = if input.starts_with('@') {
        parse_target_prefix(input)?
    } else {
        (CommandTarget::Current, input)
    };

    let command = parse_command(cmd_str)?;

    Ok(RoutedCommand { command, target })
}

fn parse_target_prefix(input: &str) -> Result<(CommandTarget, &str)> {
    let rest = &input[1..]; // skip '@'
    let (target_str, cmd_str) = rest.split_once(' ')
        .ok_or_else(|| anyhow::anyhow!("Expected command after @target"))?;

    let target = match target_str.to_lowercase().as_str() {
        "all" => CommandTarget::All,
        s if s.starts_with("group:") => {
            let id: u32 = s[6..].parse()
                .map_err(|_| anyhow::anyhow!("Invalid group ID in '{}'", s))?;
            CommandTarget::Group(id)
        }
        name => CommandTarget::Character(name.to_string()),
    };

    Ok((target, cmd_str.trim()))
}

fn parse_command(input: &str) -> Result<Command> {
    if !input.starts_with('/') {
        bail!("Commands must start with '/'");
    }

    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    let cmd = parts[0].to_lowercase();
    let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

    match cmd.as_str() {
        "/cast" => {
            let gem: u8 = args.parse()
                .map_err(|_| anyhow::anyhow!("Invalid gem number: '{}'", args))?;
            if !(1..=12).contains(&gem) {
                bail!("Gem must be 1-12, got {}", gem);
            }
            Ok(Command::Cast { gem })
        }
        "/target" => {
            if args.is_empty() { bail!("/target requires a name"); }
            Ok(Command::Target { name: args.to_string() })
        }
        "/assist" => {
            if args.is_empty() { bail!("/assist requires a name"); }
            Ok(Command::Assist { name: args.to_string() })
        }
        "/follow" => {
            if args.is_empty() { bail!("/follow requires a name"); }
            Ok(Command::Follow { name: args.to_string() })
        }
        "/key" => {
            let key = EqKey::from_name(args)
                .ok_or_else(|| anyhow::anyhow!("Unknown key: '{}'", args))?;
            Ok(Command::Key { key, shift: false, ctrl: false, alt: false })
        }
        "/sit" => Ok(Command::Sit),
        "/stand" => Ok(Command::Stand),
        "/attack" => {
            match args.to_lowercase().as_str() {
                "on" | "1" | "true" => Ok(Command::Attack(true)),
                "off" | "0" | "false" | "" => Ok(Command::Attack(false)),
                _ => bail!("Invalid /attack argument: '{}'", args),
            }
        }
        _ => {
            // Pass through as raw EQ command
            Ok(Command::EqSlash(input.to_string()))
        }
    }
}
```

- [ ] **Step 2: Write unit tests for parser**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cast_command() {
        let cmd = parse("/cast 3").unwrap();
        assert!(matches!(cmd.command, Command::Cast { gem: 3 }));
        assert!(matches!(cmd.target, CommandTarget::Current));
    }

    #[test]
    fn parse_target_command() {
        let cmd = parse("/target Lady Vox").unwrap();
        assert!(matches!(cmd.command, Command::Target { ref name } if name == "Lady Vox"));
    }

    #[test]
    fn parse_routed_command() {
        let cmd = parse("@Frostreaver /cast 1").unwrap();
        assert!(matches!(cmd.target, CommandTarget::Character(ref n) if n == "frostreaver"));
        assert!(matches!(cmd.command, Command::Cast { gem: 1 }));
    }

    #[test]
    fn parse_broadcast() {
        let cmd = parse("@all /sit").unwrap();
        assert!(matches!(cmd.target, CommandTarget::All));
        assert!(matches!(cmd.command, Command::Sit));
    }

    #[test]
    fn parse_group_command() {
        let cmd = parse("@group:2 /follow Frostreaver").unwrap();
        assert!(matches!(cmd.target, CommandTarget::Group(2)));
        assert!(matches!(cmd.command, Command::Follow { ref name } if name == "Frostreaver"));
    }

    #[test]
    fn parse_raw_eq_command() {
        let cmd = parse("/say Hello world").unwrap();
        assert!(matches!(cmd.command, Command::EqSlash(ref s) if s == "/say Hello world"));
    }

    #[test]
    fn parse_invalid_gem() {
        assert!(parse("/cast 15").is_err());
        assert!(parse("/cast abc").is_err());
    }

    #[test]
    fn parse_empty_fails() {
        assert!(parse("").is_err());
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test command::parser 2>&1`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/command/
git commit -m "feat(command): add slash command parser with routing"
```

---

## Task 8: Command Executor (`command/executor.rs`)

**Files:**

- Create: `src/command/executor.rs`
- Modify: `src/command/mod.rs` — add `pub mod executor;`

- [ ] **Step 1: Implement executor — translates Commands into KeySequences**

```rust
// src/command/executor.rs
use anyhow::Result;
use std::time::Duration;

use super::types::Command;
use crate::client::session::EqSession;
use crate::input::keys::EqKey;
use crate::input::sequence::KeySequence;

/// Execute a Command against an EqSession.
pub fn execute(cmd: &Command, session: &EqSession) -> Result<()> {
    match cmd {
        Command::EqSlash(text) => {
            session.send_command(text)
        }

        Command::Cast { gem } => {
            // EQ default keybinds: spell gems 1-8 map to keys 1-8,
            // gems 9-12 map to Ctrl+1 through Ctrl+4.
            let seq = gem_to_sequence(*gem)?;
            session.send_sequence(&seq)
        }

        Command::Target { name } => {
            session.send_command(&format!("/target {}", name))
        }

        Command::Assist { name } => {
            session.send_command(&format!("/assist {}", name))
        }

        Command::Follow { name } => {
            session.send_command(&format!("/follow {}", name))
        }

        Command::Key { key, shift, ctrl, alt } => {
            let modifiers = crate::input::keys::EqKeyModifiers {
                shift: *shift,
                ctrl: *ctrl,
                alt: *alt,
            };
            crate::input::dispatch::send_key(session.window, *key, modifiers)
        }

        Command::Sit => {
            session.send_command("/sit")
        }

        Command::Stand => {
            session.send_command("/stand")
        }

        Command::Attack(on) => {
            if *on {
                session.send_command("/attack on")
            } else {
                session.send_command("/attack off")
            }
        }
    }
}

/// Convert a spell gem number (1-12) to the appropriate key sequence.
fn gem_to_sequence(gem: u8) -> Result<KeySequence> {
    let seq = match gem {
        1 => KeySequence::new().press(EqKey::Num1),
        2 => KeySequence::new().press(EqKey::Num2),
        3 => KeySequence::new().press(EqKey::Num3),
        4 => KeySequence::new().press(EqKey::Num4),
        5 => KeySequence::new().press(EqKey::Num5),
        6 => KeySequence::new().press(EqKey::Num6),
        7 => KeySequence::new().press(EqKey::Num7),
        8 => KeySequence::new().press(EqKey::Num8),
        9 => KeySequence::new().press_modified(
            EqKey::Num1,
            crate::input::keys::EqKeyModifiers { ctrl: true, ..Default::default() },
        ),
        10 => KeySequence::new().press_modified(
            EqKey::Num2,
            crate::input::keys::EqKeyModifiers { ctrl: true, ..Default::default() },
        ),
        11 => KeySequence::new().press_modified(
            EqKey::Num3,
            crate::input::keys::EqKeyModifiers { ctrl: true, ..Default::default() },
        ),
        12 => KeySequence::new().press_modified(
            EqKey::Num4,
            crate::input::keys::EqKeyModifiers { ctrl: true, ..Default::default() },
        ),
        _ => anyhow::bail!("Invalid gem number: {}", gem),
    };
    Ok(seq)
}
```

- [ ] **Step 2: Write unit tests for gem_to_sequence**

Add to `src/command/executor.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gem_1_through_8_are_number_keys() {
        for gem in 1..=8 {
            let seq = gem_to_sequence(gem).unwrap();
            assert!(!seq.is_empty());
        }
    }

    #[test]
    fn gem_9_through_12_are_ctrl_number_keys() {
        for gem in 9..=12 {
            let seq = gem_to_sequence(gem).unwrap();
            assert!(!seq.is_empty());
        }
    }

    #[test]
    fn gem_0_is_invalid() {
        assert!(gem_to_sequence(0).is_err());
    }

    #[test]
    fn gem_13_is_invalid() {
        assert!(gem_to_sequence(13).is_err());
    }
}
```

- [ ] **Step 3: Verify build and run tests**

Run: `cargo test command::executor 2>&1`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/command/executor.rs src/command/mod.rs
git commit -m "feat(command): add executor that translates commands to key sequences"
```

---

## Task 9: TUI Integration — Command Input Bar

**Files:**

- Modify: `src/tui/app.rs` — add command input state, client manager reference
- Modify: `src/tui/event.rs` — add command input mode (`:` key to enter command mode)
- Modify: `src/tui/ui.rs` — add command input bar, client list panel

- [ ] **Step 1: Add command input state to App**

In `src/tui/app.rs`, add:

```rust
// New fields in App struct:
pub command_input: String,
pub command_mode: bool,
pub command_history: Vec<String>,
pub command_history_idx: usize,
```

Add a new `ActivePanel` variant:

```rust
pub enum ActivePanel {
    SpawnList,
    HexDump,
    ClientList,  // NEW: shows all connected EQ sessions
}
```

- [ ] **Step 2: Add command input keybindings to event.rs**

In `src/tui/event.rs`, add a command input mode:

- `:` enters command mode (like vim)
- In command mode: type command, Enter executes, Escape cancels
- Up/Down cycles through command history

- [ ] **Step 3: Add command input bar to ui.rs**

Add a command input bar at the bottom of the status bar that shows when in command mode. Show the typed command with a cursor indicator.

- [ ] **Step 4: Verify build**

Run: `cargo build 2>&1`

- [ ] **Step 5: Commit**

```bash
git add src/tui/
git commit -m "feat(tui): add command input bar and client list panel"
```

---

## Task 10: Wire Everything Together + Integration Test

**Files:**

- Create: `src/lib.rs` — re-export modules for integration tests
- Modify: `src/main.rs` — integrate ClientManager into TUI mode, use `frostreaver::` imports
- Modify: `src/tui/run.rs` — use ClientManager for data refresh
- Modify: `src/tui/app.rs` — add `client_manager: Option<ClientManager>` field

- [ ] **Step 1: Create `src/lib.rs` for crate-level exports**

The crate is currently binary-only (`main.rs`). Integration tests require a `lib.rs` to import from.

```rust
// src/lib.rs
#![allow(dead_code)]

pub mod config;
pub mod eq;
pub mod process;
pub mod input;
pub mod client;
pub mod command;
pub mod tui;
```

Then update `main.rs` to remove its own `mod` declarations and import from the lib:

```rust
use frostreaver::{config, eq, process, input, client, command, tui};
```

- [ ] **Step 2: Update TUI run loop to use ClientManager**

Add a `client_manager` field to `App` in `tui/app.rs`. In `run_tui_mode()` (main.rs), create a `ClientManager`, run `discover()`, and store it in the App. Update `refresh_eq_data()` in `tui/run.rs` to call `manager.discover()` for new processes and refresh session data.

```rust
// In tui/app.rs, add to App struct:
pub client_manager: crate::client::manager::ClientManager,

// In App::new():
client_manager: crate::client::manager::ClientManager::new(),
```

In `tui/run.rs`, update `refresh_eq_data` to call:

```rust
app.client_manager.discover().ok();
// Update app.spawns, app.local_player, app.target from the first session
```

- [ ] **Step 3: Add command input handling to TUI**

In `tui/event.rs`, add command input mode:

- `:` key enters command mode (sets `app.command_mode = true`)
- In command mode: printable chars append to `app.command_input`, Backspace deletes, Enter parses and executes via `command::parser::parse()` + `command::executor::execute()`, Escape cancels
- Up/Down cycles `app.command_history`

In `tui/ui.rs`, when `app.command_mode` is true, replace the status bar with:

```rust
fn draw_command_bar(frame: &mut Frame, area: Rect, app: &App) {
    let input = Paragraph::new(Line::from(vec![
        Span::styled(":", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(&app.command_input),
        Span::styled("█", Style::default().fg(Color::Yellow)), // cursor
    ]))
    .block(Block::default().borders(Borders::ALL).title(" Command "));
    frame.render_widget(input, area);
}
```

- [ ] **Step 4: Add integration test — command parse + execute pipeline**

Create `tests/command_integration.rs`:

```rust
use frostreaver::command::parser::parse;
use frostreaver::command::types::{Command, CommandTarget};

#[test]
fn full_command_pipeline() {
    let cmd = parse("@all /cast 5").unwrap();
    assert!(matches!(cmd.target, CommandTarget::All));
    assert!(matches!(cmd.command, Command::Cast { gem: 5 }));
}
```

- [ ] **Step 3: Run all tests**

Run: `cargo test 2>&1`
Expected: All tests pass.

- [ ] **Step 4: Final build verification**

Run: `cargo build 2>&1`
Expected: Clean compile, no warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(m2): wire up ClientManager, command pipeline, and TUI integration"
```

---

## Verification (Milestone 2)

1. `cargo build --release` succeeds with no warnings
2. `cargo test` — all parser tests pass
3. TUI launches in demo mode on macOS showing command input bar
4. On Windows with live EQ:
   - ClientManager discovers multiple eqgame.exe processes
   - Each session correctly identifies character name
   - `/target <name>` types the command in the correct EQ window
   - `/cast <gem>` sends the correct keypress to the correct window
   - `@all /sit` sends `/sit` to all connected clients
   - `@group:1 /follow Frostreaver` sends follow to all group 1 members
5. No EQ client crashes or bans (PostMessage is well-tolerated by EQ)

---

## Architecture Notes for Future Milestones

**M3 (CH Chain)** will build on M2's `KeySequence` and `ClientManager`:

- CH Chain = timed `/cast` commands staggered across 12 clerics
- Needs: spell cast time tracking (read from memory), sequence scheduling
- The `CommandTarget::Group` routing + `execute()` pipeline handle the dispatch

**M4 (IPC)** extends `ClientManager` across machines:

- Named pipes between Rust processes on different Shadow PCs
- One process per machine runs `ClientManager` for its local EQ clients
- A coordinator process routes commands across machines via IPC

**M5 (ASSIST/FOLLOW/STICK macros)** uses the command parser + spawn data:

- Read target info from memory (M1), send assist/follow commands (M2)
- `/stick` needs continuous position tracking + movement key dispatch
