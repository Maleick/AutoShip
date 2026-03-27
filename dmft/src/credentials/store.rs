use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::Connection;
use zeroize::Zeroizing;

use super::crypto;

/// Encrypted credential store backed by SQLite.
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

impl CredentialStore {
    /// Open (or create) the credential store at the given path.
    pub fn open(path: &Path, master_key: Zeroizing<[u8; 32]>) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open credential store at {}", path.display()))?;

        conn.execute_batch("PRAGMA journal_mode = WAL;")
            .context("Failed to set WAL mode on credential store")?;

        conn.execute_batch(SCHEMA)
            .context("Failed to initialize credential store schema")?;

        Ok(Self { conn: Mutex::new(conn), master_key })
    }

    /// Add or update an account's encrypted password.
    pub fn add_account(&self, account_name: &str, password: &str) -> Result<()> {
        let salt = crypto::generate_salt();
        let account_key = crypto::derive_key_from_master(&self.master_key, &salt)?;
        let (ciphertext, nonce) = crypto::encrypt(password.as_bytes(), &account_key)?;

        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("credential store mutex poisoned: {}", e))?;
        conn.execute(
            "INSERT INTO accounts (account_name, password_enc, nonce, salt, updated_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))
             ON CONFLICT(account_name) DO UPDATE SET
                password_enc = excluded.password_enc,
                nonce = excluded.nonce,
                salt = excluded.salt,
                updated_at = datetime('now')",
            rusqlite::params![account_name, ciphertext, nonce, salt.to_vec()],
        ).context("Failed to add account")?;

        Ok(())
    }

    /// Retrieve and decrypt the password for a given account.
    pub fn get_password(&self, account_name: &str) -> Result<Zeroizing<String>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("credential store mutex poisoned: {}", e))?;
        let (password_enc, nonce, salt): (Vec<u8>, Vec<u8>, Vec<u8>) = conn.query_row(
            "SELECT password_enc, nonce, salt FROM accounts WHERE account_name = ?1",
            rusqlite::params![account_name],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).with_context(|| format!("Account '{}' not found", account_name))?;

        let account_key = crypto::derive_key_from_master(&self.master_key, &salt)?;
        let plaintext = crypto::decrypt(&password_enc, &account_key, &nonce)
            .context("Failed to decrypt password")?;

        let plaintext_str = String::from_utf8(plaintext)
            .context("Decrypted password is not valid UTF-8")?;
        Ok(Zeroizing::new(plaintext_str))
    }

    /// List all stored account names.
    pub fn list_accounts(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("credential store mutex poisoned: {}", e))?;
        let mut stmt = conn.prepare("SELECT account_name FROM accounts ORDER BY account_name")?;
        let names = stmt.query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<String>, _>>()?;
        Ok(names)
    }

    /// Remove an account from the store.
    pub fn remove_account(&self, account_name: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("credential store mutex poisoned: {}", e))?;
        let rows = conn.execute(
            "DELETE FROM accounts WHERE account_name = ?1",
            rusqlite::params![account_name],
        )?;

        if rows == 0 {
            anyhow::bail!("Account '{}' not found", account_name);
        }

        Ok(())
    }
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
}
