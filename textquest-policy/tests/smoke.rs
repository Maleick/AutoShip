//! CI smoke test: promote → activate → rollback → activate again.
//! Verifies registry state and rollback_history.jsonl at each step.

use std::path::Path;

use textquest_policy::{PolicyStore, rollback::RollbackEvent};

const TEST_SIG_KEY: &str = "textquest-test-signing-key";

fn make_artifact(dir: &Path, scope: &str, version: &str, source: &str, sig_key: &str) {
    std::fs::create_dir_all(dir).unwrap();

    let data = format!("policy data for {}/{}", scope, version).into_bytes();
    std::fs::write(dir.join("policy.bin"), &data).unwrap();

    // Compute the hash that signature.rs will use (must match implementation).
    let hash = textquest_policy::signature::sha256_hex(&data);

    let manifest = serde_json::json!({
        "scope": scope,
        "version": version,
        "source_bundle": source,
        "sha256": hash,
        "produced_at": "2026-04-25T00:00:00Z",
    });
    std::fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();

    let sig = textquest_policy::signature::signature_hex(scope, version, &hash, sig_key);
    std::fs::write(dir.join("signature"), sig.as_bytes()).unwrap();
    std::fs::write(dir.join("canary_report.html"), b"<html>test canary</html>").unwrap();
}

#[test]
fn promote_activate_rollback_cycle() {
    // SAFETY: single-threaded test setup; no concurrent env access.
    unsafe { std::env::set_var("TEXTQUEST_POLICY_SIG_KEY", TEST_SIG_KEY) };

    // Ensure legacy bypass has no effect.
    // SAFETY: single-threaded test setup; no concurrent env access.
    unsafe { std::env::set_var("TEST_BYPASS_POLICY_SIG", "1") };

    let tmp = tempfile::tempdir().unwrap();
    let store_root = tmp.path().join("policies");
    let artifacts_src = tmp.path().join("artifacts_src");

    let store = PolicyStore::open(&store_root).expect("store open");

    // ── Step 1: promote v1 ──────────────────────────────────────────────────
    let src_v1 = artifacts_src.join("cleric.heal_picker.bandits.v1");
    make_artifact(
        &src_v1,
        "cleric.heal_picker",
        "bandits.v1",
        "canary#2026-04-20",
        TEST_SIG_KEY,
    );
    store.promote(&src_v1).expect("promote v1");

    // Not yet active — bundle should be rule-based default.
    let bundle = store.bundle("cleric.heal_picker");
    assert!(
        bundle.is_rule_based(),
        "before activation, bundle should be rule-based"
    );

    // ── Step 2: activate v1 ─────────────────────────────────────────────────
    store
        .activate("cleric.heal_picker", "bandits.v1")
        .expect("activate v1");

    let bundle = store.bundle("cleric.heal_picker");
    assert_eq!(bundle.version, "bandits.v1");
    assert_eq!(bundle.scope, "cleric.heal_picker");

    // Check rollback_history has one Activated event.
    let history_path = store_root.join("rollback_history.jsonl");
    assert!(
        history_path.exists(),
        "rollback_history.jsonl must exist after activate"
    );
    let history = textquest_policy::rollback::RollbackHistory::new(&history_path);
    let events = history.load().expect("load history");
    assert_eq!(events.len(), 1, "one event after first activate");
    assert!(
        matches!(&events[0], RollbackEvent::Activated { version, .. } if version == "bandits.v1"),
        "first event should be Activated for bandits.v1, got: {:?}",
        events[0]
    );

    // ── Step 3: promote v2 ──────────────────────────────────────────────────
    let src_v2 = artifacts_src.join("cleric.heal_picker.bandits.v2");
    make_artifact(
        &src_v2,
        "cleric.heal_picker",
        "bandits.v2",
        "canary#2026-04-22",
        TEST_SIG_KEY,
    );
    store.promote(&src_v2).expect("promote v2");
    store
        .activate("cleric.heal_picker", "bandits.v2")
        .expect("activate v2");

    let bundle = store.bundle("cleric.heal_picker");
    assert_eq!(bundle.version, "bandits.v2");

    // ── Step 4: rollback to v1 ──────────────────────────────────────────────
    store.rollback("cleric.heal_picker").expect("rollback");

    let bundle = store.bundle("cleric.heal_picker");
    assert_eq!(
        bundle.version, "bandits.v1",
        "after rollback, bundle should be v1"
    );

    let events = history.load().expect("load history after rollback");
    assert!(
        events.len() >= 3,
        "should have at least 3 events (activate v1, activate v2, rollback), got {}",
        events.len()
    );
    let last = events.last().unwrap();
    assert!(
        matches!(last, RollbackEvent::RolledBack { from_version, to_version, .. }
            if from_version == "bandits.v2" && to_version == "bandits.v1"),
        "last event should be RolledBack v2 → v1, got: {:?}",
        last
    );

    // ── Step 5: activate v2 again ───────────────────────────────────────────
    store
        .activate("cleric.heal_picker", "bandits.v2")
        .expect("re-activate v2");

    let bundle = store.bundle("cleric.heal_picker");
    assert_eq!(
        bundle.version, "bandits.v2",
        "after re-activate, bundle should be v2"
    );

    let events = history.load().expect("load history after re-activate");
    let last = events.last().unwrap();
    assert!(
        matches!(last, RollbackEvent::Activated { version, .. } if version == "bandits.v2"),
        "last event should be Activated for bandits.v2, got: {:?}",
        last
    );

    // Verify rollback_history.jsonl is append-only (non-empty).
    let content = std::fs::read_to_string(&history_path).unwrap();
    assert!(
        content.lines().count() >= 4,
        "JSONL should have at least 4 lines"
    );

    // ── Step 6: store survives reopen ───────────────────────────────────────
    drop(store);
    let store2 = PolicyStore::open(&store_root).expect("reopen store");
    let bundle = store2.bundle("cleric.heal_picker");
    assert_eq!(
        bundle.version, "bandits.v2",
        "bundle restored from registry on reopen"
    );
}

#[test]
fn store_rejects_invalid_signature() {
    // SAFETY: single-threaded test setup; no concurrent env access.
    unsafe { std::env::set_var("TEXTQUEST_POLICY_SIG_KEY", TEST_SIG_KEY) };

    let tmp = tempfile::tempdir().unwrap();
    let store_root = tmp.path().join("policies");
    let artifacts_src = tmp.path().join("artifacts_src");

    // Set wrong sha256 in manifest to trigger content hash failure.
    let src = artifacts_src.join("warrior.rotation.rl.v1");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("policy.bin"), b"real data").unwrap();

    let bad_manifest = serde_json::json!({
        "scope": "warrior.rotation",
        "version": "rl.v1",
        "source_bundle": "canary#bad",
        "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
        "produced_at": "2026-04-25T00:00:00Z",
    });
    std::fs::write(src.join("manifest.json"), bad_manifest.to_string()).unwrap();
    std::fs::write(src.join("signature"), b"bad-sig").unwrap();

    let store = PolicyStore::open(&store_root).expect("store open");
    let result = store.promote(&src);
    assert!(
        result.is_err(),
        "promote with wrong content hash should fail"
    );
}

#[test]
fn default_bundle_is_rule_based() {
    let tmp = tempfile::tempdir().unwrap();
    let store_root = tmp.path().join("policies");
    let store = PolicyStore::open(&store_root).expect("store open");

    let bundle = store.bundle("any.new.scope");
    assert!(
        bundle.is_rule_based(),
        "default bundle should be rule-based marker"
    );
}

#[test]
fn store_rejects_missing_sig_key_with_empty_signature() {
    // SAFETY: single-threaded test setup; no concurrent env access.
    unsafe { std::env::remove_var("TEXTQUEST_POLICY_SIG_KEY") };

    let tmp = tempfile::tempdir().unwrap();
    let store_root = tmp.path().join("policies");
    let artifacts_src = tmp.path().join("artifacts_src");

    let src = artifacts_src.join("rogue.rotation.rl.v1");
    std::fs::create_dir_all(&src).unwrap();

    let data = b"real data";
    std::fs::write(src.join("policy.bin"), data).unwrap();
    let hash = textquest_policy::signature::sha256_hex(data);
    let manifest = serde_json::json!({
        "scope": "rogue.rotation",
        "version": "rl.v1",
        "source_bundle": "canary#missing-key",
        "sha256": hash,
        "produced_at": "2026-04-25T00:00:00Z",
    });
    std::fs::write(src.join("manifest.json"), manifest.to_string()).unwrap();

    // Reproduces the bypass condition: empty signature bytes are parsed as missing.
    std::fs::write(src.join("signature"), b"").unwrap();

    let store = PolicyStore::open(&store_root).expect("store open");
    let result = store.promote(&src);
    assert!(
        result.is_err(),
        "promote should fail when signature key is unset, even with empty signature"
    );
}
