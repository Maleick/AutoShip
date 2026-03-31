//! Integration test stubs for functionality requiring a live EQ process.
//! These tests are ignored by default and serve as a testing roadmap.
//! Run with: cargo test -p dmft -- --ignored

#[test]
#[ignore = "requires live EQ process"]
fn token_file_lifecycle() {
    // TODO: Test that token_{pid}.bin is created on injection,
    // read by TUI, and consumed by DLL. Verify the token file
    // is cleaned up after the DLL acknowledges receipt.
}

#[test]
#[ignore = "requires live EQ process"]
fn pipe_auth_handshake() {
    // TODO: Test named pipe connection, auth token exchange,
    // and command/response roundtrip. Verify that an invalid
    // token is rejected and a valid token establishes a session.
}

#[test]
#[ignore = "requires live EQ process"]
fn camp_start_dispatch() {
    // TODO: Test that camp start creates sessions for all clients,
    // generates IPC commands (buff requests, pull assignments),
    // and dispatches them to the correct client pipes.
}

#[test]
#[ignore = "requires live EQ process"]
fn spawn_list_reading() {
    // TODO: Test that the spawn linked list is walked correctly,
    // producing valid SpawnInfo entries with reasonable field values.
    // Verify the max-count safety limit prevents infinite loops on
    // corrupted NEXT pointers.
}

#[test]
#[ignore = "requires live EQ process"]
fn dll_injection_and_hook_install() {
    // TODO: Test that the DLL is injected into a running eqgame.exe,
    // hooks are installed (ProcessGameEvents, movement), and the
    // self-healing monitor detects and recovers from hook failures.
}

#[test]
#[ignore = "requires live EQ process"]
fn group_membership_sync() {
    // TODO: Test that live EQ group membership is read correctly,
    // GroupInfo structs are populated with leader and member names,
    // and the Groups TUI screen reflects the actual in-game groups.
}

#[test]
#[ignore = "requires live EQ process"]
fn credential_store_roundtrip() {
    // TODO: Test the encrypted credential store end-to-end:
    // store credentials (Argon2id + AES-256-GCM), retrieve them,
    // verify decryption produces the original values, and confirm
    // the SQLite backend persists across process restarts.
}

#[test]
#[ignore = "requires live EQ process"]
fn zone_routing_pathfind() {
    // TODO: Test that the zone router computes a valid multi-zone
    // path (e.g., Permafrost -> Eastern Wastes via zone connections),
    // generates waypoints for each zone transition, and the Navigator
    // FSM follows the path with stuck detection active.
}
