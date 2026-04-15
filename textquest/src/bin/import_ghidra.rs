//! Import a local Ghidra JSON cache into the runtime/debug GhidraDatabase
//! (SQLite).
//!
//! Usage: import_ghidra [DB_PATH] [JSON_DIR]
//!   DB_PATH  — SQLite cache path (default: data/ghidra.db)
//!   JSON_DIR — Directory containing local Ghidra export cache files (default:
//! data/ghidra-export/)
//!
//! Immutable manifests and snapshot evidence stay canonical in the sibling
//! `Maleick/TextQuest-Ghidra` repo; this tool only hydrates local runtime/debug
//! state.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use textquest_common::ghidra_db::{FunctionEntry, GhidraDatabase, ImportEntry, StringEntry};

#[derive(Deserialize)]
struct JsonFunction {
    name: String,
    address: String,
}

#[derive(Deserialize)]
struct JsonString {
    address: String,
    value: String,
}

#[derive(Deserialize)]
struct JsonImport {
    name: String,
    address: String,
}

#[derive(Deserialize)]
struct JsonBookmark {
    address: String,
    category: String,
    comment: String,
    #[serde(rename = "type")]
    bookmark_type: String,
}

#[derive(Deserialize)]
struct JsonMetadata {
    #[serde(flatten)]
    fields: serde_json::Map<String, serde_json::Value>,
}

fn parse_hex_addr(s: &str) -> Result<u64> {
    let stripped = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u64::from_str_radix(stripped, 16).with_context(|| format!("invalid hex address: {s}"))
}

fn parse_import_addr(s: &str) -> Result<u64> {
    if let Some(hex_part) = s.strip_prefix("EXTERNAL:") {
        u64::from_str_radix(hex_part, 16).with_context(|| format!("invalid import address: {s}"))
    } else {
        parse_hex_addr(s)
    }
}

fn load_functions(dir: &Path, db: &GhidraDatabase) -> Result<usize> {
    let path = dir.join("functions.json");
    if !path.exists() {
        eprintln!("  skipping functions.json (not found)");
        return Ok(0);
    }
    let data = std::fs::read_to_string(&path)?;
    let raw: Vec<JsonFunction> = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let entries: Vec<FunctionEntry> = raw
        .into_iter()
        .filter_map(|f| {
            let addr = parse_hex_addr(&f.address).ok()?;
            Some(FunctionEntry {
                address: addr,
                name: f.name,
                size: None,
                category: Some("ghidra_auto".into()),
                source: Some("ghidra".into()),
                description: None,
                usability: None,
                notes: None,
            })
        })
        .collect();
    db.import_functions(&entries)
}

fn load_strings(dir: &Path, db: &GhidraDatabase) -> Result<usize> {
    let path = dir.join("strings.json");
    if !path.exists() {
        eprintln!("  skipping strings.json (not found)");
        return Ok(0);
    }
    let data = std::fs::read_to_string(&path)?;
    let raw: Vec<JsonString> = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let entries: Vec<StringEntry> = raw
        .into_iter()
        .filter_map(|s| {
            let addr = parse_hex_addr(&s.address).ok()?;
            Some(StringEntry {
                address: addr,
                value: s.value,
                ref_function_addr: None,
            })
        })
        .collect();
    db.import_strings(&entries)
}

fn load_imports(dir: &Path, db: &GhidraDatabase) -> Result<usize> {
    let path = dir.join("imports.json");
    if !path.exists() {
        eprintln!("  skipping imports.json (not found)");
        return Ok(0);
    }
    let data = std::fs::read_to_string(&path)?;
    let raw: Vec<JsonImport> = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let entries: Vec<ImportEntry> = raw
        .into_iter()
        .filter_map(|i| {
            let addr = parse_import_addr(&i.address).ok()?;
            Some(ImportEntry {
                address: addr,
                dll_name: "unknown".into(),
                func_name: i.name,
            })
        })
        .collect();
    db.import_imports(&entries)
}

fn load_bookmarks(dir: &Path, db: &GhidraDatabase) -> Result<usize> {
    let path = dir.join("bookmarks.json");
    if !path.exists() {
        eprintln!("  skipping bookmarks.json (not found)");
        return Ok(0);
    }
    let data = std::fs::read_to_string(&path)?;
    let raw: Vec<JsonBookmark> = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let entries: Vec<FunctionEntry> = raw
        .into_iter()
        .filter_map(|b| {
            let addr = parse_hex_addr(&b.address).ok()?;
            Some(FunctionEntry {
                address: addr,
                name: format!("[{}] {}", b.category, b.comment),
                size: None,
                category: Some(format!("bookmark:{}", b.category)),
                source: Some(format!("ghidra:{}", b.bookmark_type)),
                description: Some(b.comment),
                usability: None,
                notes: None,
            })
        })
        .collect();
    db.import_functions(&entries)
}

fn load_classes(dir: &Path, db: &GhidraDatabase) -> Result<usize> {
    let path = dir.join("classes.json");
    if !path.exists() {
        eprintln!("  skipping classes.json (not found)");
        return Ok(0);
    }
    let data = std::fs::read_to_string(&path)?;
    let raw: Vec<String> = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let classes_json = serde_json::to_string(&raw)?;
    db.set_metadata("rtti_classes", &classes_json)?;
    db.set_metadata("rtti_class_count", &raw.len().to_string())?;
    Ok(raw.len())
}

fn load_metadata(dir: &Path, db: &GhidraDatabase) -> Result<usize> {
    let path = dir.join("metadata.json");
    if !path.exists() {
        eprintln!("  skipping metadata.json (not found)");
        return Ok(0);
    }
    let data = std::fs::read_to_string(&path)?;
    let raw: JsonMetadata = serde_json::from_str(&data)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let mut count = 0;
    for (key, value) in &raw.fields {
        let val_str = match value {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        db.set_metadata(&format!("harvest_{key}"), &val_str)?;
        count += 1;
    }
    Ok(count)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let db_path = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| "data/ghidra.db".into());
    let json_dir = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| "data/ghidra-export".into());

    if !json_dir.is_dir() {
        bail!("JSON directory not found: {}", json_dir.display());
    }
    if let Some(parent) = db_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }

    println!("Opening local database cache: {}", db_path.display());
    let db = GhidraDatabase::open(&db_path)?;
    println!("Importing local export cache from: {}", json_dir.display());
    println!();

    let func_count = load_functions(&json_dir, &db)?;
    println!("  functions:  {func_count}");
    let str_count = load_strings(&json_dir, &db)?;
    println!("  strings:    {str_count}");
    let imp_count = load_imports(&json_dir, &db)?;
    println!("  imports:    {imp_count}");
    let bm_count = load_bookmarks(&json_dir, &db)?;
    println!("  bookmarks:  {bm_count} (stored as annotated functions)");
    let cls_count = load_classes(&json_dir, &db)?;
    println!("  classes:    {cls_count} (stored as metadata)");
    let meta_count = load_metadata(&json_dir, &db)?;
    println!("  metadata:   {meta_count} keys");

    println!();
    let stats = db.stats()?;
    println!("=== Database Summary ===");
    println!("  functions:      {}", stats.functions);
    println!("  function_calls: {}", stats.function_calls);
    println!("  globals:        {}", stats.globals);
    println!("  opcodes:        {}", stats.opcodes);
    println!("  strings:        {}", stats.strings);
    println!("  imports:        {}", stats.imports);
    println!();
    println!("Done. Local database cache saved to {}", db_path.display());
    Ok(())
}
