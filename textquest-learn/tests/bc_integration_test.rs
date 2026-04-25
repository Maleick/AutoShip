use textquest_learn::{
    bookmarks::BookmarkStore, ledger::ExperienceLedger, quality_gates::QualityGates,
    onnx_export::OnnxExporter, FlaggedSegmentDataset, PolicyMetadata, training::BehaviorCloningTrainer,
};
use tempfile::TempDir;

#[test]
fn test_behavior_cloning_pipeline() {
    // Create temporary directories
    let temp_ledger = TempDir::new().unwrap();
    let temp_db = TempDir::new().unwrap();

    let ledger_path = temp_ledger.path();
    let db_path = temp_db.path().join("bookmarks.db");

    // Create fixture ledger data
    create_fixture_ledger(ledger_path);

    // Create fixture bookmarks database
    create_fixture_bookmarks(&db_path);

    // Load ledger and bookmarks
    let ledger = ExperienceLedger::load_from_dir(ledger_path).unwrap();
    assert!(!ledger.all_entries().is_empty(), "Ledger should have entries");

    let bookmarks = BookmarkStore::load_from_sqlite(&db_path).unwrap();
    assert!(!bookmarks.get_bookmarks().is_empty(), "Bookmarks should exist");

    // Build dataset for cleric class
    let dataset = FlaggedSegmentDataset::build(&ledger, &bookmarks, "cleric", "v3").unwrap();
    assert!(dataset.pairs.len() > 0, "Dataset should have training pairs");
    assert!(dataset.context_dim() > 0, "Context dimension should be > 0");

    let (train_pairs, heldout_pairs) = dataset.split_train_heldout(0.2);
    assert!(train_pairs.len() > 0, "Training pairs should exist");
    assert!(heldout_pairs.len() > 0, "Heldout pairs should exist");

    // Train model
    let trainer = BehaviorCloningTrainer::new(0.01, 10, 16);
    let model = trainer.train(&dataset).unwrap();
    assert_eq!(model.class, "cleric");
    assert_eq!(model.context_dim, dataset.context_dim());

    // Evaluate held-out performance
    let metrics = QualityGates::evaluate_heldout(&model, &heldout_pairs).unwrap();
    println!(
        "Action match rate: {:.2}% ({}/{})",
        metrics.action_match_rate * 100.0,
        metrics.correct_predictions,
        metrics.total_samples
    );

    // Check coverage
    let coverage = QualityGates::check_coverage(&train_pairs, &heldout_pairs).unwrap();
    println!("Coverage: {} well-covered clusters", coverage.well_covered_clusters);
    println!("Underfitted: {:?}", coverage.underfitted_clusters);

    // Export model
    let output_path = temp_db.path().join("cleric.bc.v1.onnx");
    let metadata = PolicyMetadata {
        class: "cleric".to_string(),
        training_data_manifest: vec![("session_1".to_string(), (0, 10))],
        context_schema_version: "v3".to_string(),
        exporter_git_sha: "test_sha".to_string(),
        held_out_action_match_rate: metrics.action_match_rate,
        total_training_samples: train_pairs.len(),
        total_heldout_samples: heldout_pairs.len(),
        underfitted_context_clusters: coverage.underfitted_clusters,
    };

    OnnxExporter::export(&model, &metadata, &output_path).unwrap();
    assert!(output_path.exists(), "Exported model should exist");

    // Verify inference latency requirement
    let latency = OnnxExporter::inference_latency();
    assert!(latency <= 200, "Inference latency {} μs exceeds 200 μs", latency);
}

fn create_fixture_ledger(ledger_path: &std::path::Path) {
    use serde_json::json;
    std::fs::create_dir_all(ledger_path).unwrap();

    let mut entries = Vec::new();

    // Generate fixture data: 10 sessions, 20 entries each, actions 0-5
    for session_idx in 0..2 {
        let session_id = format!("session_{}", session_idx);
        for entry_idx in 0..50 {
            let context = json!({
                "hp_ratio": 0.5 + (entry_idx as f32 * 0.01),
                "mana_ratio": 0.3 + (entry_idx as f32 * 0.005),
                "enemy_distance": 20.0 - (entry_idx as f32 * 0.2),
                "buff_count": (entry_idx % 5) as f32,
            });

            entries.push(serde_json::json!({
                "session_id": session_id,
                "timestamp": (session_idx * 1000 + entry_idx),
                "class": if session_idx % 2 == 0 { "cleric" } else { "warrior" },
                "context": context,
                "action": (entry_idx % 6) as u32,
                "reward": 0.8 + (entry_idx as f32 * 0.01),
            }));
        }
    }

    let ledger_data = entries
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("\n");

    std::fs::write(ledger_path.join("fixtures.jsonl"), ledger_data).unwrap();
}

fn create_fixture_bookmarks(db_path: &std::path::Path) {
    use rusqlite::Connection;

    let conn = Connection::open(db_path).unwrap();
    conn.execute(
        "CREATE TABLE bookmarks (
            session_id TEXT,
            start_flag_idx INTEGER,
            end_flag_idx INTEGER,
            label TEXT
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO bookmarks (session_id, start_flag_idx, end_flag_idx, label)
         VALUES (?, ?, ?, ?)",
        ["session_0", "0", "30", "good"],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO bookmarks (session_id, start_flag_idx, end_flag_idx, label)
         VALUES (?, ?, ?, ?)",
        ["session_1", "10", "40", "good"],
    )
    .unwrap();
}
