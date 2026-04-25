# Result: #2270 — Wire packet monitor UI to real data — remove 5 hardcoded stubs in ui/packets.rs
- Wired packet stream/detail UI rendering to `PacketRecord` helpers for payload preview and client labels.
- Added `PacketMonitorState::resolved_client_label` to support process-name lookup for filter bar labels.
- Kept peak/selection/rate display driven by `PacketMonitorState` and removed remaining hardcoded rendering paths.
- Ran `cargo check` successfully.
