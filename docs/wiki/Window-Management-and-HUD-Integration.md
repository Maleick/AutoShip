# Window Management and HUD Integration

TextQuest exposes a shared window-management contract in
`textquest_common::window_management`. The first implementation pass provides a
stable JSON-backed protocol for external HUD tools and runtime adapters. Native
Win32 movement hooks, monitor discovery, and renderer-specific transparent
overlay work remain follow-up tasks.

## API Surface

External tools submit a `WindowState` through `ExternalHudWindowApi`:

- `WindowState` identifies the window, profile, owner, visibility, theme, and
  display mode.
- `WindowPlacement` contains monitor selection, anchor, offset, size, z-order,
  focus policy, and opacity.
- `DisplaySurface` describes one monitor in virtual desktop coordinates.
- `WindowLayoutProfile` stores per-profile display and window state.
- `JsonWindowLayoutStore` imports and exports profile JSON files.
- `PersistentWindowManager` wires the one-method external API to profile
  persistence.

Anchors are resolved against a selected monitor with fallback to the primary
display and then the first known display. This supports top-left, edge-center,
center, and bottom-right style HUD placement without external tools needing to
know final virtual desktop coordinates.

## External Tool Example

```rust
use textquest_common::window_management::{
    DisplayMode, DisplaySurface, ExternalHudWindowApi, FocusPolicy, JsonWindowLayoutStore,
    OverlayOpacity, PersistentWindowManager, WindowAnchor, WindowOffset, WindowPlacement,
    WindowPoint, WindowSize, WindowState, WindowZOrder,
};

let store = JsonWindowLayoutStore::new("config/window-layouts");
let displays = vec![DisplaySurface {
    id: "primary".into(),
    name: Some("Main".into()),
    origin: WindowPoint { x: 0, y: 0 },
    size: WindowSize {
        width: 1920,
        height: 1080,
    },
    scale_factor_millis: 1000,
    primary: true,
}];
let mut api = PersistentWindowManager::load_or_new(store, "default", displays)?;

let mut hud = WindowState::external_hud("mq2hudmove-main", "default");
hud.display_mode = DisplayMode::ClickThroughOverlay;
hud.placement = WindowPlacement {
    monitor_id: Some("primary".into()),
    anchor: WindowAnchor::BottomRight,
    offset: WindowOffset { x: -24, y: -24 },
    size: WindowSize {
        width: 420,
        height: 240,
    },
    z_order: WindowZOrder::TopMost,
    focus_policy: FocusPolicy::ClickThrough,
    opacity: OverlayOpacity::new(700),
};

let ack = api.apply_window_state(hud)?;
```

The returned acknowledgement includes the resolved rectangle and confirms that
the updated profile was persisted.

## Current Scope

This pass is intentionally partial for issue #857. It establishes the shared
protocol, anchoring model, profile persistence, and documentation needed for
external HUD compatibility. Remaining work should add runtime display discovery,
native window move/resize/focus operations, transparent overlay rendering, and
validated examples against real external HUD tools.
