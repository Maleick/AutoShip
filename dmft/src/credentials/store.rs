use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;

use super::crypto;

/// Encrypted credential store backed by SQLite.
pub struct CredentialStore {
    conn: Connection,
    master_key: [u8; 32],
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
    pub fn open(path: &Path, master_key: [u8; 32]) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open credential store at {}", path.display()))?;

        conn.execute_batch(SCHEMA)
            .context("Failed to initialize credential store schema")?;

        Ok(Self { conn, master_key })
    }

    /// Add or update an account's encrypted password.
    pub fn add_account(&self, account_name: &str, password: &str) -> Result<()> {
        let salt = crypto::generate_salt();
        let (ciphertext, nonce) = crypto::encrypt(password.as_bytes(), &self.master_key);

        self.conn.execute(
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
    pub fn get_password(&self, account_name: &str) -> Result<String> {
        let (password_enc, nonce): (Vec<u8>, Vec<u8>) = self.conn.query_row(
            "SELECT password_enc, nonce FROM accounts WHERE account_name = ?1",
            rusqlite::params![account_name],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).with_context(|| format!("Account '{}' not found", account_name))?;

        let plaintext = crypto::decrypt(&password_enc, &self.master_key, &nonce)
            .context("Failed to decrypt password")?;

        String::from_utf8(plaintext).context("Decrypted password is not valid UTF-8")
    }

    /// List all stored account names.
    pub fn list_accounts(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT account_name FROM accounts ORDER BY account_name")?;
        let names = stmt.query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<String>, _>>()?;
        Ok(names)
    }

    /// Remove an account from the store.
    pub fn remove_account(&self, account_name: &str) -> Result<()> {
        let rows = self.conn.execute(
            "DELETE FROM accounts WHERE account_name = ?1",
            rusqlite::params![account_name],
        )?;

        if rows == 0 {
            anyhow::bail!("Account '{}' not found", account_name);
        }

        Ok(())
    }
}
