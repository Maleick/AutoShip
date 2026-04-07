//! In-memory account registry with encrypted password storage.
//!
//! Account metadata (name, server, character, class, group, status) is held in an
//! `Arc<Mutex<AccountStore>>` that lives in `AppState`.  Passwords are encrypted with
//! AES-256-GCM using a per-account key derived from the master key via Argon2id, and
//! persisted in the shared `data/credentials.db` SQLite database — the same schema
//! used by the CLI credential store in the orchestrator crate.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, OsRng},
};
use anyhow::{Context, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

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

/// A single EQ account record (metadata only — no plaintext password stored here).
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

    /// Update an existing account.  Returns the updated record or an error if not found.
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

// ─── Credential store ─────────────────────────────────────────────────────────

/// Schema for the credential store — intentionally identical to the orchestrator's schema
/// so both tools can share `data/credentials.db`.
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
    // Parameters match the orchestrator's credential store (textquest/src/credentials/crypto.rs):
    //   m_cost  = 65536 KiB (64 MiB memory cost)
    //   t_cost  = 3     (time / iteration count)
    //   p_cost  = 4     (parallelism)
    //   output  = 32    bytes (256-bit key for AES-256-GCM)
    let params = Params::new(65536, 3, 4, Some(32))
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

#[allow(dead_code)]
fn aes_decrypt(ciphertext: &[u8], key: &[u8; 32], nonce_bytes: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(key.into());
    if nonce_bytes.len() != 12 {
        anyhow::bail!("nonce must be exactly 12 bytes, got {}", nonce_bytes.len());
    }
    let mut arr = [0u8; 12];
    arr.copy_from_slice(nonce_bytes);
    let nonce = Nonce::from(arr);
    cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("Decryption failed: {e}"))
}

/// Thin wrapper around a shared SQLite credential database.
pub struct CredentialStore {
    conn: Mutex<Connection>,
    master_key: Zeroizing<[u8; 32]>,
}

impl CredentialStore {
    /// Open (or create) the credential store at the given path.
    pub fn open(path: &Path, master_password: &str) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open credential store at {}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode = WAL;")
            .context("Failed to set WAL mode on credential store")?;
        conn.execute_batch(CREDENTIAL_SCHEMA)
            .context("Failed to initialise credential store schema")?;

        let salt = generate_salt();
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

// ─── Constructor helpers ──────────────────────────────────────────────────────

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

#[cfg(test)]
mod tests {
    use super::*;

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
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("creds.db");
        let cred_store = CredentialStore::open(&db_path, "test_master_pw").unwrap();
        assert!(!cred_store.has_password("acct1").unwrap());
        cred_store.set_password("acct1", "hunter2").unwrap();
        assert!(cred_store.has_password("acct1").unwrap());
    }

    #[test]
    fn credential_store_remove_password() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("creds.db");
        let cred_store = CredentialStore::open(&db_path, "master").unwrap();
        cred_store.set_password("acct2", "pass").unwrap();
        cred_store.remove_password("acct2").unwrap();
        assert!(!cred_store.has_password("acct2").unwrap());
    }
}
