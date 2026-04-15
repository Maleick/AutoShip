use std::{path::Path, sync::Mutex};

use anyhow::{Context, Result};
use rusqlite::Connection;
use zeroize::Zeroizing;

use super::crypto;

/// Encrypted credential store backed by `SQLite`.
pub struct CredentialStore {
    conn: Mutex<Connection>,
    master_key: Zeroizing<[u8; 32]>,
}

const SCHEMA: &str = "
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

/// Default on-disk credential store path used by CLI and unattended relog
/// flows.
pub const DEFAULT_CREDENTIAL_DB_PATH: &str = "data/credentials.db";
const DEFAULT_CREDENTIAL_META_TABLE: &str = "credential_store_meta";

impl CredentialStore {
    /// Open (or create) the credential store at the given path.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn open(path: &Path, master_key: Zeroizing<[u8; 32]>) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open credential store at {}", path.display()))?;

        conn.execute_batch("PRAGMA journal_mode = WAL;")
            .context("Failed to set WAL mode on credential store")?;

        conn.execute_batch(SCHEMA)
            .context("Failed to initialize credential store schema")?;

        Ok(Self {
            conn: Mutex::new(conn),
            master_key,
        })
    }

    /// Add or update an account's encrypted password.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn add_account(&self, account_name: &str, password: impl AsRef<[u8]>) -> Result<()> {
        let salt = crypto::generate_salt();
        let account_key = crypto::derive_key_from_master(&self.master_key, &salt)?;
        let (ciphertext, nonce) = crypto::encrypt(password.as_ref(), &account_key)?;

        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("credential store mutex poisoned: {e}"))?;
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
        .context("Failed to add account")?;

        Ok(())
    }

    /// Retrieve and decrypt the password for a given account.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn get_password(&self, account_name: &str) -> Result<Zeroizing<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("credential store mutex poisoned: {e}"))?;
        let (password_enc, nonce, salt): (Vec<u8>, Vec<u8>, Vec<u8>) = conn
            .query_row(
                "SELECT password_enc, nonce, salt FROM accounts WHERE account_name = ?1",
                rusqlite::params![account_name],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .with_context(|| format!("Account '{account_name}' not found"))?;

        let account_key = crypto::derive_key_from_master(&self.master_key, &salt)?;
        let plaintext = crypto::decrypt(&password_enc, &account_key, &nonce)
            .context("Failed to decrypt password")?;

        let plaintext_str =
            String::from_utf8(plaintext).context("Decrypted password is not valid UTF-8")?;
        Ok(Zeroizing::new(plaintext_str))
    }

    /// List all stored account names.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn list_accounts(&self) -> Result<Vec<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("credential store mutex poisoned: {e}"))?;
        let mut stmt = conn.prepare("SELECT account_name FROM accounts ORDER BY account_name")?;
        let names = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<String>, _>>()?;
        Ok(names)
    }

    /// Remove an account from the store.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn remove_account(&self, account_name: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("credential store mutex poisoned: {e}"))?;
        let rows = conn.execute(
            "DELETE FROM accounts WHERE account_name = ?1",
            rusqlite::params![account_name],
        )?;

        if rows == 0 {
            anyhow::bail!("Account '{account_name}' not found");
        }

        Ok(())
    }

    /// Open the default on-disk credential store using a master password
    /// string.
    ///
    /// This derives the master key, creates the metadata table if needed, and
    /// reuses the same DB path as the interactive credential-management CLI.
    ///
    /// # Errors
    ///
    /// Returns an error if the store cannot be opened or the key material
    /// cannot be derived.
    pub fn open_default(master_password: &str) -> Result<Self> {
        let db_path = std::path::PathBuf::from(DEFAULT_CREDENTIAL_DB_PATH);
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let salt = load_or_create_master_salt(&db_path)?;
        let master_key = crypto::derive_key(master_password, &salt)?;
        Self::open(&db_path, master_key)
    }
}

fn load_or_create_master_salt(db_path: &Path) -> Result<[u8; 32]> {
    use rusqlite::OptionalExtension;

    let conn = Connection::open(db_path).with_context(|| {
        format!(
            "Failed to open credential metadata DB at {}",
            db_path.display()
        )
    })?;
    conn.execute(
        &format!(
            "CREATE TABLE IF NOT EXISTS {DEFAULT_CREDENTIAL_META_TABLE} (key TEXT PRIMARY KEY, \
             value BLOB NOT NULL)"
        ),
        [],
    )
    .context("Failed to ensure credential metadata table exists")?;

    let existing: Option<Vec<u8>> = conn
        .query_row(
            &format!("SELECT value FROM {DEFAULT_CREDENTIAL_META_TABLE} WHERE key = 'master_salt'"),
            [],
            |row| row.get(0),
        )
        .optional()
        .context("Failed to query credential store master salt")?;

    if let Some(bytes) = existing {
        if bytes.len() != 32 {
            anyhow::bail!("Credential store master salt has invalid length");
        }
        let mut salt = [0u8; 32];
        salt.copy_from_slice(&bytes);
        return Ok(salt);
    }

    let salt = crypto::generate_salt();
    conn.execute(
        &format!(
            "INSERT INTO {DEFAULT_CREDENTIAL_META_TABLE} (key, value) VALUES ('master_salt', ?1)"
        ),
        rusqlite::params![salt.to_vec()],
    )
    .context("Failed to persist credential store master salt")?;
    Ok(salt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_master_key() -> Zeroizing<[u8; 32]> {
        let salt = crypto::generate_salt();
        crypto::derive_key("test_master_password", &salt).unwrap()
    }

    fn open_memory_store() -> CredentialStore {
        let path = PathBuf::from(":memory:");
        CredentialStore::open(&path, test_master_key()).unwrap()
    }

    #[test]
    fn open_in_memory_succeeds() {
        let store = open_memory_store();
        let accounts = store.list_accounts().unwrap();
        assert!(accounts.is_empty());
    }

    #[test]
    fn open_with_temp_file_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("creds.db");
        let store = CredentialStore::open(&db_path, test_master_key()).unwrap();
        let accounts = store.list_accounts().unwrap();
        assert!(accounts.is_empty());
    }

    #[test]
    fn add_then_get_password_roundtrip() {
        let store = open_memory_store();
        store.add_account("warrior_acct", "hunter2").unwrap();

        let password = store.get_password("warrior_acct").unwrap();
        assert_eq!(&*password, "hunter2");
    }

    #[test]
    fn add_multiple_accounts_and_list() {
        let store = open_memory_store();
        store.add_account("alpha", "pass_a").unwrap();
        store.add_account("bravo", "pass_b").unwrap();
        store.add_account("charlie", "pass_c").unwrap();

        let mut accounts = store.list_accounts().unwrap();
        accounts.sort();
        assert_eq!(accounts, vec!["alpha", "bravo", "charlie"]);
    }

    #[test]
    fn add_account_upserts_on_duplicate() {
        let store = open_memory_store();
        store.add_account("warrior_acct", "old_password").unwrap();
        store.add_account("warrior_acct", "new_password").unwrap();

        let password = store.get_password("warrior_acct").unwrap();
        assert_eq!(&*password, "new_password");

        let accounts = store.list_accounts().unwrap();
        assert_eq!(accounts.len(), 1);
    }

    #[test]
    fn remove_account_removes_it() {
        let store = open_memory_store();
        store.add_account("to_remove", "pass").unwrap();
        store.remove_account("to_remove").unwrap();

        let accounts = store.list_accounts().unwrap();
        assert!(accounts.is_empty());
    }

    #[test]
    fn remove_nonexistent_account_fails() {
        let store = open_memory_store();
        let result = store.remove_account("does_not_exist");
        assert!(result.is_err());
    }

    #[test]
    fn get_password_nonexistent_account_fails() {
        let store = open_memory_store();
        let result = store.get_password("no_such_account");
        assert!(result.is_err());
    }

    #[test]
    fn add_account_with_empty_password() {
        let store = open_memory_store();
        store.add_account("empty_pass", "").unwrap();
        let password = store.get_password("empty_pass").unwrap();
        assert_eq!(&*password, "");
    }

    #[test]
    fn add_account_with_special_characters() {
        let store = open_memory_store();
        let special = "p@$$w0rd!#%^&*(){}[]|\\:\";<>,.?/~`";
        store.add_account("special", special).unwrap();
        let password = store.get_password("special").unwrap();
        assert_eq!(&*password, special);
    }

    #[test]
    fn add_account_with_unicode() {
        let store = open_memory_store();
        let unicode_pass = "password_\u{1F600}_\u{00E9}";
        store.add_account("unicode_acct", unicode_pass).unwrap();
        let password = store.get_password("unicode_acct").unwrap();
        assert_eq!(&*password, unicode_pass);
    }

    #[test]
    fn list_accounts_sorted() {
        let store = open_memory_store();
        store.add_account("charlie", "pass").unwrap();
        store.add_account("alpha", "pass").unwrap();
        store.add_account("bravo", "pass").unwrap();
        let accounts = store.list_accounts().unwrap();
        assert_eq!(accounts, vec!["alpha", "bravo", "charlie"]);
    }

    #[test]
    fn remove_and_re_add_account() {
        let store = open_memory_store();
        store.add_account("reuse", "first_pass").unwrap();
        store.remove_account("reuse").unwrap();
        store.add_account("reuse", "second_pass").unwrap();
        let password = store.get_password("reuse").unwrap();
        assert_eq!(&*password, "second_pass");
    }

    #[test]
    fn get_password_after_remove_fails() {
        let store = open_memory_store();
        store.add_account("temp", "pass").unwrap();
        store.remove_account("temp").unwrap();
        assert!(store.get_password("temp").is_err());
    }

    #[test]
    fn add_account_with_long_password() {
        let store = open_memory_store();
        let long_pass = "a".repeat(10_000);
        store.add_account("long_pass_acct", &long_pass).unwrap();
        let password = store.get_password("long_pass_acct").unwrap();
        assert_eq!(&*password, &*long_pass);
    }

    #[test]
    fn add_account_accepts_zeroizing_password() {
        let store = open_memory_store();
        let password = Zeroizing::new("zeroized_pass".to_string());

        store.add_account("zeroized", password).unwrap();

        let stored = store.get_password("zeroized").unwrap();
        assert_eq!(&*stored, "zeroized_pass");
    }
}
