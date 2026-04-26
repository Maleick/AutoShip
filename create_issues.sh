#!/bin/bash
# Create TBD issues from issue #1181 task breakdown

set -e

cd /Users/maleick/Projects/TextQuest/.claude/worktrees/pedantic-roentgen-6f5575/.autoship/workspaces/issue-1181

# Theme System
gh issue create --title "Apply theme to all TUI components" \
  --milestone "TUI Polish - Theme System" \
  --label task --label ui --label theme-system \
  --body "Implement theme color application across all TUI rendering functions.

Dependencies: #1133 (Create theme TOML files and loader)

Acceptance Criteria:
- All text colors use theme palette
- All background colors use theme palette
- Theme changes apply to all components without full UI rebuild
- No hardcoded colors in rendering code"

gh issue create --title "Add theme toggle command" \
  --milestone "TUI Polish - Theme System" \
  --label task --label ui --label theme-system \
  --body "Implement user command to switch between available themes.

Dependencies: Apply theme to all TUI components

Acceptance Criteria:
- Command switches between all available themes
- Theme change updates UI immediately
- Available themes listed in help
- Default theme applied on startup"

gh issue create --title "Persist theme selection" \
  --milestone "TUI Polish - Theme System" \
  --label task --label infrastructure --label theme-system \
  --body "Save user's theme choice to config and restore on startup.

Dependencies: Add theme toggle command

Acceptance Criteria:
- Selected theme written to config file
- Theme restored from config on app startup
- Invalid theme selection falls back to default
- Config format matches TOML structure"

# Metrics System
gh issue create --title "Wire collector into orchestrator" \
  --milestone "TUI Polish - Metrics System" \
  --label task --label infrastructure --label metrics \
  --body "Connect metrics collector to main orchestrator event loop.

Dependencies: #1150 (Implement metrics collector & aggregation)

Acceptance Criteria:
- Collector receives events from orchestrator
- Events properly typed and formatted
- No orchestrator performance regression
- Metrics are actively accumulated"

gh issue create --title "Hook combat/movement/loot events" \
  --milestone "TUI Polish - Metrics System" \
  --label task --label infrastructure --label metrics \
  --body "Emit events from combat, movement, and loot systems to metrics collector.

Dependencies: Wire collector into orchestrator

Acceptance Criteria:
- Combat damage/heals tracked
- Movement distance calculated
- Loot pickups recorded with item name/value
- Events fire with correct timestamp
- No impact on gameplay performance"

gh issue create --title "Render metrics panel in TUI" \
  --milestone "TUI Polish - Metrics System" \
  --label task --label ui --label metrics \
  --body "Display real-time metrics dashboard in TUI.

Dependencies: Hook combat/movement/loot events

Acceptance Criteria:
- Panel shows DPS, movement speed, loot value/hour
- Metrics update in real-time
- Panel resizable and movable
- Metrics reset on command
- UI responsive when metrics generated at high frequency"

gh issue create --title "Add metrics persistence (SQLite)" \
  --milestone "TUI Polish - Metrics System" \
  --label task --label infrastructure --label metrics \
  --body "Store historical metrics data in SQLite for analysis.

Dependencies: Render metrics panel in TUI

Acceptance Criteria:
- Metrics written to SQLite on interval
- Historical data can be queried
- Data retention policy enforced (e.g., 30 days)
- No blocking I/O on metrics thread"

# Audio & Hotkeys
gh issue create --title "Create default alert sounds" \
  --milestone "TUI Polish - Audio & Hotkeys" \
  --label task --label assets --label audio-alerts \
  --body "Create or source audio files for common alert conditions.

Dependencies: #1178 (Implement audio alert playback)

Acceptance Criteria:
- Alert sound for low health
- Alert sound for buff expiration
- Alert sound for mob aggro
- Alert sound for inventory full
- All sounds are .wav or .ogg, <1MB each"

gh issue create --title "Wire audio to event system" \
  --milestone "TUI Polish - Audio & Hotkeys" \
  --label task --label infrastructure --label audio-alerts \
  --body "Connect audio playback to game event stream.

Dependencies: Create default alert sounds

Acceptance Criteria:
- Audio plays when configured events occur
- Multiple sounds can play simultaneously
- Volume control functional
- No event processing lag from audio playback"

gh issue create --title "Add hotkey conflict detection" \
  --milestone "TUI Polish - Audio & Hotkeys" \
  --label task --label ui --label hotkeys \
  --body "Detect and warn when hotkey bindings conflict.

Dependencies: #1179 (Implement hotkey registry & routing)

Acceptance Criteria:
- Conflicts detected when user binds existing key
- User warned before binding
- Existing binding shown
- Option to override or cancel
- Built-in hotkeys cannot be overridden"

gh issue create --title "Create hotkey configuration UI" \
  --milestone "TUI Polish - Audio & Hotkeys" \
  --label task --label ui --label hotkeys \
  --body "Build TUI interface for customizing hotkey bindings.

Dependencies: Add hotkey conflict detection

Acceptance Criteria:
- List all available commands
- Show current bindings
- Allow rebind with validation
- Save/revert changes
- Import/export binding profiles"

# Keyboard & Accessibility
gh issue create --title "Define keyboard shortcut map" \
  --milestone "Keyboard & Accessibility" \
  --label task --label documentation --label accessibility \
  --body "Document all keyboard shortcuts and create default keybinding map.

Acceptance Criteria:
- TOML file defining all shortcuts
- Shortcuts organized by category (navigation, combat, UI)
- Default bindings for common editors (Vi, Emacs)
- ASCII art map showing key positions
- Exported to help system"

gh issue create --title "Add Vi keybinding mode" \
  --milestone "Keyboard & Accessibility" \
  --label task --label ui --label accessibility \
  --body "Implement Vi-style keybindings throughout TUI.

Dependencies: Define keyboard shortcut map

Acceptance Criteria:
- hjkl movement
- w/b word navigation in search
- / and ? search forward/back
- : command entry
- u/ctrl-r undo/redo
- Activable from settings"

gh issue create --title "Add Emacs keybinding mode" \
  --milestone "Keyboard & Accessibility" \
  --label task --label ui --label accessibility \
  --body "Implement Emacs-style keybindings throughout TUI.

Dependencies: Define keyboard shortcut map

Acceptance Criteria:
- ctrl-n/p/f/b movement
- ctrl-a/e line start/end
- ctrl-k/u kill/undo line
- alt-b/f word navigation
- Search with ctrl-s/r
- Activable from settings"

gh issue create --title "Improve focus indicators" \
  --milestone "Keyboard & Accessibility" \
  --label task --label ui --label accessibility \
  --body "Add visible focus indicators for keyboard navigation.

Acceptance Criteria:
- Active focus element clearly highlighted
- Highlight works in all UI components
- Accessible color contrast (WCAG AA)
- Focus outline doesn't obscure content
- Tab order is logical and documented"

gh issue create --title "Add high-contrast mode" \
  --milestone "Keyboard & Accessibility" \
  --label task --label ui --label accessibility \
  --body "Implement high-contrast color theme for visibility.

Acceptance Criteria:
- WCAG AAA contrast ratios for all text
- Distinct colors for colorblind users
- Themed borders and indicators
- Bold fonts in high-contrast mode
- Activable from settings"

gh issue create --title "Document all shortcuts" \
  --milestone "Keyboard & Accessibility" \
  --label task --label documentation --label accessibility \
  --body "Create comprehensive keyboard shortcut documentation.

Acceptance Criteria:
- User guide listing all shortcuts by mode
- In-game help showing current keybindings
- PDF export of shortcut reference
- Printable cheat sheet
- Accessible from ? key"

# Web Frontend - Foundation
gh issue create --title "Set up form handling & validation" \
  --milestone "Web Frontend - Foundation" \
  --label task --label infrastructure --label web-frontend \
  --body "Implement form utilities for handling and validating user input.

Dependencies: #1160 (Set up Vite + React project structure)

Acceptance Criteria:
- Form validation library integrated
- Required field validation
- Email/number/text validators
- Custom validation rules
- Error message display
- Form state management"

gh issue create --title "Configure TypeScript & linting" \
  --milestone "Web Frontend - Foundation" \
  --label task --label infrastructure --label web-frontend \
  --body "Set up TypeScript, ESLint, and Prettier for code quality.

Dependencies: #1160 (Set up Vite + React project structure)

Acceptance Criteria:
- TypeScript strict mode enabled
- ESLint configured with React rules
- Prettier auto-formatting on save
- Pre-commit hooks enforce linting
- CI checks linting pass"

gh issue create --title "Add dark mode theming" \
  --milestone "Web Frontend - Foundation" \
  --label task --label ui --label web-frontend \
  --body "Implement dark/light theme toggle for web dashboard.

Dependencies: #1166 (Build reusable React component library)

Acceptance Criteria:
- Theme provider wraps app
- CSS variables for theme colors
- User preference persisted to localStorage
- OS preference detection
- Smooth theme transition
- All components support both themes"

# Web Frontend - Credentials & Groups
gh issue create --title "Build navigation & routing" \
  --milestone "Web Frontend - Credentials & Groups" \
  --label task --label infrastructure --label web-frontend \
  --body "Implement React Router for multi-page navigation.

Dependencies: #1160 (Set up Vite + React project structure)

Acceptance Criteria:
- Routes for each dashboard page
- Navigation bar/menu
- Breadcrumbs for navigation context
- Back/forward browser buttons work
- Deep linking supported
- 404 page for invalid routes"

gh issue create --title "Add form persistence" \
  --milestone "Web Frontend - Credentials & Groups" \
  --label task --label infrastructure --label web-frontend \
  --body "Persist form state to localStorage with auto-save.

Dependencies: #1171 (Build Credentials Management page), #1173 (Build Groups & Camp Configuration page)

Acceptance Criteria:
- Form state saved on change
- Auto-save indicator shown
- Unsaved changes warning on page exit
- Clear cache option
- Drafts recoverable after crash"

gh issue create --title "Implement API integration" \
  --milestone "Web Frontend - Credentials & Groups" \
  --label task --label infrastructure --label web-frontend \
  --body "Wire frontend forms to backend API endpoints.

Dependencies: #1165 (Build API client library), #1171 (Build Credentials Management page), #1173 (Build Groups & Camp Configuration page)

Acceptance Criteria:
- Forms submit to correct endpoints
- API responses handled correctly
- Error messages from API displayed
- Loading states shown
- Retry logic on network failure
- CSRF protection implemented"

# Web Frontend - Monitoring
gh issue create --title "Implement real-time event updates" \
  --milestone "Web Frontend - Monitoring" \
  --label task --label infrastructure --label web-frontend \
  --body "Handle real-time event stream from WebSocket.

Dependencies: #1174 (Build Session Monitoring dashboard)

Acceptance Criteria:
- Events processed as received
- UI updated <500ms after event
- Backpressure handling for high-frequency events
- Event queue doesn't grow unbounded
- Reconnect on connection loss"

gh issue create --title "Add alert feed & notifications" \
  --milestone "Web Frontend - Monitoring" \
  --label task --label ui --label web-frontend \
  --body "Display alert feed and browser notifications for events.

Dependencies: #1174 (Build Session Monitoring dashboard)

Acceptance Criteria:
- Alert feed shows recent events
- Browser notification on critical events
- Notification settings (enable/disable by type)
- Alert TTL (fade after 5 minutes)
- Alert click navigates to relevant section"

gh issue create --title "Create responsive mobile layout" \
  --milestone "Web Frontend - Monitoring" \
  --label task --label ui --label web-frontend \
  --body "Make all dashboard pages mobile-responsive.

Dependencies: All web frontend pages

Acceptance Criteria:
- Mobile breakpoint (480px) with stacked layout
- Touch-friendly button sizes (48px+)
- No horizontal scroll
- Modal-based menus on mobile
- Images scale appropriately
- Tested on iOS and Android"

# Error Diagnostics & Debugger
gh issue create --title "Add diagnostic panel to TUI" \
  --milestone "Error Diagnostics & Debugger" \
  --label task --label ui --label debugging \
  --body "Build TUI panel showing system diagnostics and errors.

Acceptance Criteria:
- Recent error log display
- System resource usage (CPU, memory)
- Network status indicator
- Active threads/tasks list
- Error histogram by type
- Panel togglable with hotkey"

gh issue create --title "Wire error context throughout" \
  --milestone "Error Diagnostics & Debugger" \
  --label task --label infrastructure --label debugging \
  --body "Capture and propagate error context throughout codebase.

Acceptance Criteria:
- Error struct includes stack trace, context, suggestions
- All error sites wrapped with context
- Context persisted to error log
- No performance overhead
- Error ID generation for reporting"

gh issue create --title "Add error recovery suggestions" \
  --milestone "Error Diagnostics & Debugger" \
  --label task --label ui --label debugging \
  --body "Display recovery suggestions when errors occur.

Acceptance Criteria:
- Network error suggests reconnect
- Permission error suggests account check
- Timeout error suggests retry
- Custom suggestions for known errors
- One-click recovery actions"

gh issue create --title "Integrate debugger into TUI" \
  --milestone "Error Diagnostics & Debugger" \
  --label task --label ui --label debugging \
  --body "Integrate debugger tool into TUI interface.

Dependencies: #1000 (Extract debugger from test branch)

Acceptance Criteria:
- Debugger breakpoints work
- Variable inspection functional
- Step through code in TUI
- Debugger hotkey to pause game
- Debugger output captured"

gh issue create --title "Add packet monitor UI" \
  --milestone "Error Diagnostics & Debugger" \
  --label task --label ui --label debugging \
  --body "Build TUI display for EQ packet monitoring.

Acceptance Criteria:
- Show recent packets sent/received
- Filter by packet type
- Display packet structure
- Timestamp and size shown
- Export packet log
- Toggle record on/off"

echo "All 31 issues created successfully!"
