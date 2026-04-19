//! In-memory account registry with encrypted password storage.
//!
//! This module backs the web dashboard account-management slice.
//!
//! Passwords are encrypted with AES-256-GCM using a per-account key derived
//! from the master key via Argon2id, and persisted in the shared
//! `data/credentials.db` SQLite database — the same schema used by the CLI
//! credential store in the orchestrator crate.

use std::{
    collections::HashMap,
    path::Path as FsPath,
    sync::{Mutex, MutexGuard},
};

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, OsRng, rand_core::RngCore},
};
use anyhow::{Context, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post, put},
};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{AppState, api::ErrorResponse};

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ErrorResponse>)>;

#[cfg(test)]
const ARGON2_MEMORY_KIB: u32 = 8 * 1024;
#[cfg(test)]
const ARGON2_TIME_COST: u32 = 1;
#[cfg(test)]
const ARGON2_PARALLELISM: u32 = 1;

#[cfg(not(test))]
const ARGON2_MEMORY_KIB: u32 = 65_536;
#[cfg(not(test))]
const ARGON2_TIME_COST: u32 = 3;
#[cfg(not(test))]
const ARGON2_PARALLELISM: u32 = 4;

#[derive(Debug, Deserialize)]
pub struct ImportAccountsRequest {
    pub accounts: Vec<AccountRecord>,
}

#[derive(Debug, Serialize)]
pub struct ImportAccountsResponse {
    pub imported: usize,
}

#[derive(Debug, Serialize)]
pub struct ExportAccountsResponse {
    pub accounts: Vec<AccountRecord>,
}

// ─── Domain types ────────────────────────────────────────────────────────────

/// Lifecycle status of an EQ account.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountStatus {
    #[default]
    Active,
    Locked,
    Banned,
}

/// A single EQ account record (metadata only — no plaintext password stored
/// here).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountRecord {
    /// Stable identifier (UUID v4).
    pub id: String,
    /// EQ account login name.
    pub name: String,
    /// Target server (e.g., "Firiona Vie").
    pub server: String,
    /// Character name to log in as.
    pub character: String,
    /// Short class code (e.g., "WAR", "CLR").
    #[serde(default = "default_class")]
    pub class: String,
    /// Group ID this account belongs to (0 = ungrouped).
    #[serde(default)]
    pub group: u32,
    /// Account lifecycle status.
    #[serde(default)]
    pub status: AccountStatus,
    /// Whether an encrypted password is stored in the credential DB.
    #[serde(default)]
    pub has_password: bool,
}

fn default_class() -> String {
    "UNK".to_string()
}

/// Payload for creating a new account.
#[derive(Debug, Deserialize)]
pub struct CreateAccountRequest {
    pub name: String,
    pub server: String,
    pub character: String,
    #[serde(default = "default_class")]
    pub class: String,
    #[serde(default)]
    pub group: u32,
    #[serde(default)]
    pub status: AccountStatus,
    /// Optional plaintext password (never stored in plaintext).
    pub password: Option<String>,
}

/// Payload for updating an existing account.
#[derive(Debug, Deserialize)]
pub struct UpdateAccountRequest {
    pub server: Option<String>,
    pub character: Option<String>,
    pub class: Option<String>,
    pub group: Option<u32>,
    pub status: Option<AccountStatus>,
    /// Supply to change the stored password.
    pub password: Option<String>,
}

/// Payload for setting / replacing a password.
#[derive(Debug, Deserialize)]
pub struct SetPasswordRequest {
    pub password: String,
}

// ─── In-memory account store ─────────────────────────────────────────────────

/// In-memory account registry keyed by account name.
#[derive(Default)]
pub struct AccountStore {
    accounts: HashMap<String, AccountRecord>,
}

impl AccountStore {
    /// List all accounts, sorted by name.
    pub fn list(&self) -> Vec<AccountRecord> {
        let mut list: Vec<AccountRecord> = self.accounts.values().cloned().collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }

    /// Get a single account by name.
    pub fn get(&self, name: &str) -> Option<&AccountRecord> {
        self.accounts.get(name)
    }

    /// Insert a new account.  Returns an error if the name is already taken.
    pub fn insert(&mut self, rec: AccountRecord) -> Result<()> {
        if self.accounts.contains_key(&rec.name) {
            anyhow::bail!("Account '{}' already exists", rec.name);
        }
        self.accounts.insert(rec.name.clone(), rec);
        Ok(())
    }

    /// Update an existing account.  Returns the updated record or an error if
    /// not found.
    pub fn update(&mut self, name: &str, req: &UpdateAccountRequest) -> Result<AccountRecord> {
        let rec = self
            .accounts
            .get_mut(name)
            .with_context(|| format!("Account '{name}' not found"))?;
        if let Some(s) = &req.server {
            rec.server.clone_from(s);
        }
        if let Some(c) = &req.character {
            rec.character.clone_from(c);
        }
        if let Some(c) = &req.class {
            rec.class.clone_from(c);
        }
        if let Some(g) = req.group {
            rec.group = g;
        }
        if let Some(s) = &req.status {
            rec.status = s.clone();
        }
        Ok(rec.clone())
    }

    /// Mark an account as having (or not having) a stored password.
    pub fn set_has_password(&mut self, name: &str, flag: bool) -> Result<()> {
        let rec = self
            .accounts
            .get_mut(name)
            .with_context(|| format!("Account '{name}' not found"))?;
        rec.has_password = flag;
        Ok(())
    }

    /// Remove an account.  Returns an error if not found.
    pub fn remove(&mut self, name: &str) -> Result<AccountRecord> {
        self.accounts
            .remove(name)
            .with_context(|| format!("Account '{name}' not found"))
    }

    /// Bulk-import a list of account records, skipping duplicates.
    /// Returns the count of newly added accounts.
    pub fn import_bulk(&mut self, records: Vec<AccountRecord>) -> usize {
        let mut added = 0usize;
        for mut rec in records {
            if !self.accounts.contains_key(&rec.name) {
                rec.id = Uuid::new_v4().to_string();
                rec.has_password = false; // passwords are not exported
                self.accounts.insert(rec.name.clone(), rec);
                added += 1;
            }
        }
        added
    }
}

// ─── Credential store
// ─────────────────────────────────────────────────────────

/// Schema for the credential store — intentionally identical to the
/// orchestrator's schema so both tools can share `data/credentials.db`.
const CREDENTIAL_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS accounts (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    account_name TEXT NOT NULL UNIQUE,
    password_enc BLOB NOT NULL,
    nonce       BLOB NOT NULL,
    salt        BLOB NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

fn argon2_instance() -> Result<Argon2<'static>> {
    // Parameters match the orchestrator's credential store
    // (textquest/src/credentials/crypto.rs):   m_cost  = 65536 KiB (64 MiB
    // memory cost)   t_cost  = 3     (time / iteration count)
    //   p_cost  = 4     (parallelism)
    //   output  = 32    bytes (256-bit key for AES-256-GCM)
    //
    // Test builds exercise password routes end-to-end and become extremely
    // slow under tarpaulin with the production KDF settings.
    let params = Params::new(
        ARGON2_MEMORY_KIB,
        ARGON2_TIME_COST,
        ARGON2_PARALLELISM,
        Some(32),
    )
    .map_err(|e| anyhow::anyhow!("invalid argon2 params: {e}"))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

fn derive_key(master_password: &str, salt: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
    let argon2 = argon2_instance()?;
    let mut key = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(master_password.as_bytes(), salt, &mut *key)
        .map_err(|e| anyhow::anyhow!("argon2 key derivation failed: {e}"))?;
    Ok(key)
}

fn derive_key_from_master(master_key: &[u8; 32], salt: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
    let argon2 = argon2_instance()?;
    let mut key = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(master_key, salt, &mut *key)
        .map_err(|e| anyhow::anyhow!("argon2 per-account key derivation failed: {e}"))?;
    Ok(key)
}

fn generate_salt() -> [u8; 32] {
    let mut salt = [0u8; 32];
    OsRng.fill_bytes(&mut salt);
    salt
}

const MASTER_SALT_META_KEY: &str = "master_salt_hex";

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn decode_hex_32(hex: &str) -> Result<[u8; 32]> {
    fn hex_nibble(ch: u8) -> Option<u8> {
        match ch {
            b'0'..=b'9' => Some(ch - b'0'),
            b'a'..=b'f' => Some(ch - b'a' + 10),
            b'A'..=b'F' => Some(ch - b'A' + 10),
            _ => None,
        }
    }

    if hex.len() != 64 {
        anyhow::bail!(
            "invalid persisted master salt length: expected 64 hex chars, got {}",
            hex.len()
        );
    }

    let mut salt = [0u8; 32];
    let bytes = hex.as_bytes();
    for i in 0..32 {
        let hi = hex_nibble(bytes[i * 2]).with_context(|| {
            format!(
                "invalid hex character '{}' in persisted master salt",
                bytes[i * 2] as char
            )
        })?;
        let lo = hex_nibble(bytes[i * 2 + 1]).with_context(|| {
            format!(
                "invalid hex character '{}' in persisted master salt",
                bytes[i * 2 + 1] as char
            )
        })?;
        salt[i] = (hi << 4) | lo;
    }
    Ok(salt)
}

fn load_or_create_master_salt(conn: &Connection) -> Result<[u8; 32]> {
    let existing = conn
        .query_row(
            "SELECT value FROM meta WHERE key = ?1",
            rusqlite::params![MASTER_SALT_META_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .context("Failed to query master salt metadata")?;

    if let Some(salt_hex) = existing {
        return decode_hex_32(&salt_hex);
    }

    let salt = generate_salt();
    let salt_hex = encode_hex(&salt);
    conn.execute(
        "INSERT OR IGNORE INTO meta (key, value) VALUES (?1, ?2)",
        rusqlite::params![MASTER_SALT_META_KEY, salt_hex],
    )
    .context("Failed to persist master salt metadata")?;

    let persisted = conn
        .query_row(
            "SELECT value FROM meta WHERE key = ?1",
            rusqlite::params![MASTER_SALT_META_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .context("Failed to query persisted master salt metadata")?
        .context("Master salt metadata missing after insert attempt")?;

    decode_hex_32(&persisted)
}

fn aes_encrypt(plaintext: &[u8], key: &[u8; 32]) -> Result<(Vec<u8>, Vec<u8>)> {
    let cipher = Aes256Gcm::new(key.into());
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from(nonce_bytes);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| anyhow::anyhow!("AES-256-GCM encryption failed: {e}"))?;
    Ok((ciphertext, nonce_bytes.to_vec()))
}

/// Thin wrapper around a shared SQLite credential database.
pub struct CredentialStore {
    conn: Mutex<Connection>,
    master_key: Zeroizing<[u8; 32]>,
}

impl CredentialStore {
    /// Open (or create) the credential store at the given path.
    pub fn open(path: &FsPath, master_password: &str) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open credential store at {}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode = WAL;")
            .context("Failed to set WAL mode on credential store")?;
        conn.execute_batch(CREDENTIAL_SCHEMA)
            .context("Failed to initialise credential store schema")?;

        let salt = load_or_create_master_salt(&conn)?;
        let master_key = derive_key(master_password, &salt)?;
        Ok(Self {
            conn: Mutex::new(conn),
            master_key,
        })
    }

    /// Add or update an account's encrypted password.
    pub fn set_password(&self, account_name: &str, password: &str) -> Result<()> {
        let salt = generate_salt();
        let account_key = derive_key_from_master(&self.master_key, &salt)?;
        let (ciphertext, nonce) = aes_encrypt(password.as_bytes(), &account_key)?;
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        conn.execute(
            "INSERT INTO accounts (account_name, password_enc, nonce, salt, updated_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))
             ON CONFLICT(account_name) DO UPDATE SET
                password_enc = excluded.password_enc,
                nonce = excluded.nonce,
                salt = excluded.salt,
                updated_at = datetime('now')",
            rusqlite::params![account_name, ciphertext, nonce, salt.to_vec()],
        )
        .context("Failed to store password")?;
        Ok(())
    }

    /// Remove an account's password from the store.
    pub fn remove_password(&self, account_name: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        conn.execute(
            "DELETE FROM accounts WHERE account_name = ?1",
            rusqlite::params![account_name],
        )
        .context("Failed to remove password")?;
        Ok(())
    }

    /// Return `true` if a password row exists for the given account.
    pub fn has_password(&self, account_name: &str) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM accounts WHERE account_name = ?1",
            rusqlite::params![account_name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}

// ─── Constructor helpers
// ──────────────────────────────────────────────────────

/// Build an `AccountRecord` from a create request, generating a fresh UUID.
pub fn build_record(req: CreateAccountRequest) -> AccountRecord {
    AccountRecord {
        id: Uuid::new_v4().to_string(),
        name: req.name,
        server: req.server,
        character: req.character,
        class: req.class,
        group: req.group,
        status: req.status,
        has_password: false,
    }
}

pub fn router() -> Router<std::sync::Arc<AppState>> {
    Router::new()
        .route("/", get(list_accounts).post(create_account))
        .route("/export", get(export_accounts))
        .route("/import", post(import_accounts))
        .route("/{name}", put(update_account).delete(delete_account))
        .route("/{name}/password", put(set_password))
}

fn json_error(status: StatusCode, message: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            error: message.into(),
        }),
    )
}

fn lock_store(
    state: &AppState,
) -> Result<MutexGuard<'_, AccountStore>, (StatusCode, Json<ErrorResponse>)> {
    state.account_store.lock().map_err(|error| {
        json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("account store lock poisoned: {error}"),
        )
    })
}

fn require_credential_store(
    state: &AppState,
) -> Result<&CredentialStore, (StatusCode, Json<ErrorResponse>)> {
    state.credential_store.as_ref().ok_or_else(|| {
        json_error(
            StatusCode::NOT_IMPLEMENTED,
            "Password storage requires TEXTQUEST_MASTER_PASSWORD at server startup",
        )
    })
}

pub async fn list_accounts(
    State(state): State<std::sync::Arc<AppState>>,
) -> ApiResult<Vec<AccountRecord>> {
    let store = lock_store(&state)?;
    Ok(Json(store.list()))
}

pub async fn create_account(
    State(state): State<std::sync::Arc<AppState>>,
    Json(req): Json<CreateAccountRequest>,
) -> ApiResult<AccountRecord> {
    let requested_password = req.password.clone();
    if requested_password.is_some() {
        let _ = require_credential_store(&state)?;
    }

    let mut record = build_record(req);
    {
        let mut store = lock_store(&state)?;
        store
            .insert(record.clone())
            .map_err(|error| json_error(StatusCode::CONFLICT, error.to_string()))?;
    }

    if let Some(password) = requested_password.as_deref() {
        let credential_store = require_credential_store(&state)?;
        if let Err(error) = credential_store.set_password(&record.name, password) {
            let mut store = lock_store(&state)?;
            let _ = store.remove(&record.name);
            return Err(json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
            ));
        }
        let mut store = lock_store(&state)?;
        store
            .set_has_password(&record.name, true)
            .map_err(|error| json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        record.has_password = true;
    }

    Ok(Json(record))
}

pub async fn update_account(
    State(state): State<std::sync::Arc<AppState>>,
    Path(name): Path<String>,
    Json(req): Json<UpdateAccountRequest>,
) -> ApiResult<AccountRecord> {
    if req.password.is_some() {
        let _ = require_credential_store(&state)?;
    }

    let updated = {
        let mut store = lock_store(&state)?;
        store
            .update(&name, &req)
            .map_err(|error| json_error(StatusCode::NOT_FOUND, error.to_string()))?
    };

    if let Some(password) = req.password.as_deref() {
        let credential_store = require_credential_store(&state)?;
        credential_store
            .set_password(&name, password)
            .map_err(|error| json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        let mut store = lock_store(&state)?;
        store
            .set_has_password(&name, true)
            .map_err(|error| json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        let refreshed = store.get(&name).cloned().ok_or_else(|| {
            json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Account '{name}' disappeared after password update"),
            )
        })?;
        return Ok(Json(refreshed));
    }

    Ok(Json(updated))
}

pub async fn delete_account(
    State(state): State<std::sync::Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    {
        let mut store = lock_store(&state)?;
        store
            .remove(&name)
            .map_err(|error| json_error(StatusCode::NOT_FOUND, error.to_string()))?;
    }

    if let Some(credential_store) = state.credential_store.as_ref() {
        credential_store
            .remove_password(&name)
            .map_err(|error| json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn set_password(
    State(state): State<std::sync::Arc<AppState>>,
    Path(name): Path<String>,
    Json(req): Json<SetPasswordRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    {
        let store = lock_store(&state)?;
        if store.get(&name).is_none() {
            return Err(json_error(
                StatusCode::NOT_FOUND,
                format!("Account '{name}' not found"),
            ));
        }
    }

    let credential_store = require_credential_store(&state)?;
    credential_store
        .set_password(&name, &req.password)
        .map_err(|error| json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    let mut store = lock_store(&state)?;
    store
        .set_has_password(&name, true)
        .map_err(|error| json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn export_accounts(
    State(state): State<std::sync::Arc<AppState>>,
) -> ApiResult<ExportAccountsResponse> {
    let store = lock_store(&state)?;
    Ok(Json(ExportAccountsResponse {
        accounts: store.list(),
    }))
}

pub async fn import_accounts(
    State(state): State<std::sync::Arc<AppState>>,
    Json(req): Json<ImportAccountsRequest>,
) -> ApiResult<ImportAccountsResponse> {
    let mut store = lock_store(&state)?;
    let imported = store.import_bulk(req.accounts);
    Ok(Json(ImportAccountsResponse { imported }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex as StdMutex, MutexGuard as StdMutexGuard, OnceLock};

    fn credential_store_test_guard() -> StdMutexGuard<'static, ()> {
        static LOCK: OnceLock<StdMutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| StdMutex::new(()))
            .lock()
            .expect("credential store test lock poisoned")
    }

    fn make_store() -> AccountStore {
        AccountStore::default()
    }

    fn sample_record(name: &str) -> AccountRecord {
        AccountRecord {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            server: "Firiona Vie".to_string(),
            character: "Warrior".to_string(),
            class: "WAR".to_string(),
            group: 1,
            status: AccountStatus::Active,
            has_password: false,
        }
    }

    #[test]
    fn insert_and_list() {
        let mut store = make_store();
        store.insert(sample_record("alpha")).unwrap();
        store.insert(sample_record("bravo")).unwrap();
        let list = store.list();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "alpha");
    }

    #[test]
    fn insert_duplicate_fails() {
        let mut store = make_store();
        store.insert(sample_record("dupe")).unwrap();
        assert!(store.insert(sample_record("dupe")).is_err());
    }

    #[test]
    fn remove_existing() {
        let mut store = make_store();
        store.insert(sample_record("to_remove")).unwrap();
        store.remove("to_remove").unwrap();
        assert_eq!(store.list().len(), 0);
    }

    #[test]
    fn remove_missing_fails() {
        let mut store = make_store();
        assert!(store.remove("ghost").is_err());
    }

    #[test]
    fn update_fields() {
        let mut store = make_store();
        store.insert(sample_record("acct")).unwrap();
        let req = UpdateAccountRequest {
            server: Some("Rizlona".to_string()),
            character: None,
            class: None,
            group: Some(2),
            status: Some(AccountStatus::Locked),
            password: None,
        };
        let updated = store.update("acct", &req).unwrap();
        assert_eq!(updated.server, "Rizlona");
        assert_eq!(updated.group, 2);
        assert_eq!(updated.status, AccountStatus::Locked);
    }

    #[test]
    fn list_sorted_by_name() {
        let mut store = make_store();
        store.insert(sample_record("charlie")).unwrap();
        store.insert(sample_record("alpha")).unwrap();
        store.insert(sample_record("bravo")).unwrap();
        let names: Vec<_> = store.list().iter().map(|r| r.name.clone()).collect();
        assert_eq!(names, vec!["alpha", "bravo", "charlie"]);
    }

    #[test]
    fn import_bulk_skips_duplicates() {
        let mut store = make_store();
        store.insert(sample_record("existing")).unwrap();
        let to_import = vec![sample_record("existing"), sample_record("new_one")];
        let added = store.import_bulk(to_import);
        assert_eq!(added, 1);
        assert_eq!(store.list().len(), 2);
    }

    #[test]
    fn credential_store_set_and_has_password() {
        let _guard = credential_store_test_guard();
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("creds.db");
        let cred_store = CredentialStore::open(&db_path, "test_master_pw").unwrap();
        assert!(!cred_store.has_password("acct1").unwrap());
        cred_store.set_password("acct1", "hunter2").unwrap();
        assert!(cred_store.has_password("acct1").unwrap());
    }

    #[test]
    fn credential_store_remove_password() {
        let _guard = credential_store_test_guard();
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("creds.db");
        let cred_store = CredentialStore::open(&db_path, "master").unwrap();
        cred_store.set_password("acct2", "pass").unwrap();
        cred_store.remove_password("acct2").unwrap();
        assert!(!cred_store.has_password("acct2").unwrap());
    }

    #[test]
    fn credential_store_persists_master_salt_across_reopen() {
        let _guard = credential_store_test_guard();
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("creds.db");
        let _store = CredentialStore::open(&db_path, "master").unwrap();
        let conn = Connection::open(&db_path).unwrap();
        let first: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                rusqlite::params![MASTER_SALT_META_KEY],
                |row| row.get(0),
            )
            .unwrap();
        drop(conn);

        let _store = CredentialStore::open(&db_path, "master").unwrap();
        let conn = Connection::open(&db_path).unwrap();
        let second: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                rusqlite::params![MASTER_SALT_META_KEY],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(first, second);
    }
}
