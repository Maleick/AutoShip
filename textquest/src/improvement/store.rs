//! SQLite-backed posterior store for persisting Bayesian distributions across sessions.

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::bayesian::{BayesianPosterior, BetaBinomial, Gaussian};

/// Serialized posterior state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PosteriorSnapshot {
    pub knob_type: String,
    pub alpha: Option<f64>,
    pub beta: Option<f64>,
    pub mean_val: Option<f64>,
    pub variance: Option<f64>,
    pub obs_variance: Option<f64>,
    pub sum_obs: Option<f64>,
    pub count_obs: Option<f64>,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS posteriors (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    character       TEXT NOT NULL,
    knob            TEXT NOT NULL,
    knob_type       TEXT NOT NULL,
    alpha           REAL,
    beta            REAL,
    mean_val        REAL,
    variance        REAL,
    obs_variance    REAL,
    sum_obs         REAL,
    count_obs       REAL,
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(character, knob)
);
CREATE INDEX IF NOT EXISTS idx_posteriors_char ON posteriors(character);
CREATE INDEX IF NOT EXISTS idx_posteriors_knob ON posteriors(character, knob);
";

/// SQLite-backed store for Bayesian posteriors.
pub struct PosteriorStore {
    conn: Mutex<Connection>,
}

impl PosteriorStore {
    /// Open (or create) the posterior database at the given path.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open posteriors DB: {path:?}"))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .context("Failed to set PRAGMA")?;
        conn.execute_batch(SCHEMA)
            .context("Failed to initialize posteriors schema")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Open an in-memory posterior database (for testing).
    #[cfg(test)]
    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .context("Failed to open in-memory posteriors DB")?;
        conn.execute_batch(SCHEMA)
            .context("Failed to initialize posteriors schema")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Load a Beta-Binomial posterior for the given character and knob.
    pub fn load_beta_binomial(&self, character: &str, knob: &str) -> Result<BetaBinomial> {
        let conn = self.conn.lock().unwrap();
        let snapshot = conn
            .query_row(
                "SELECT alpha, beta FROM posteriors WHERE character = ?1 AND knob = ?2",
                params![character, knob],
                |row| {
                    let alpha: f64 = row.get(0)?;
                    let beta: f64 = row.get(1)?;
                    Ok((alpha, beta))
                },
            )
            .optional()
            .context("Failed to query Beta-Binomial posterior")?;

        Ok(match snapshot {
            Some((alpha, beta)) => BetaBinomial::with_prior(alpha, beta),
            None => BetaBinomial::new(),
        })
    }

    /// Load a Gaussian posterior for the given character and knob.
    pub fn load_gaussian(
        &self,
        character: &str,
        knob: &str,
        obs_variance: f64,
    ) -> Result<Gaussian> {
        let conn = self.conn.lock().unwrap();
        let snapshot = conn
            .query_row(
                "SELECT mean_val, variance FROM posteriors WHERE character = ?1 AND knob = ?2",
                params![character, knob],
                |row| {
                    let mean_val: f64 = row.get(0)?;
                    let variance: f64 = row.get(1)?;
                    Ok((mean_val, variance))
                },
            )
            .optional()
            .context("Failed to query Gaussian posterior")?;

        Ok(match snapshot {
            Some((mean_val, variance)) => {
                let mut g = Gaussian::with_prior(mean_val, variance, obs_variance);
                // Restore sum_obs and count_obs if available
                conn.query_row(
                    "SELECT sum_obs, count_obs FROM posteriors WHERE character = ?1 AND knob = ?2",
                    params![character, knob],
                    |row| {
                        g.sum_obs = row.get(0).unwrap_or(0.0);
                        g.count_obs = row.get(1).unwrap_or(0.0);
                        Ok(())
                    },
                )
                .ok();
                g
            }
            None => Gaussian::with_variance(obs_variance),
        })
    }

    /// Save a Beta-Binomial posterior.
    pub fn save_beta_binomial(
        &self,
        character: &str,
        knob: &str,
        posterior: &BetaBinomial,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO posteriors (character, knob, knob_type, alpha, beta, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
            params![character, knob, "beta_binomial", posterior.alpha(), posterior.beta()],
        )
        .context("Failed to save Beta-Binomial posterior")?;
        Ok(())
    }

    /// Save a Gaussian posterior.
    pub fn save_gaussian(
        &self,
        character: &str,
        knob: &str,
        posterior: &Gaussian,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO posteriors (character, knob, knob_type, mean_val, variance, obs_variance, sum_obs, count_obs, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))",
            params![
                character,
                knob,
                "gaussian",
                posterior.mean(),
                posterior.variance,
                posterior.obs_variance,
                posterior.sum_obs,
                posterior.count_obs,
            ],
        )
        .context("Failed to save Gaussian posterior")?;
        Ok(())
    }

    /// List all posteriors for a character.
    pub fn list_knobs(&self, character: &str) -> Result<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT knob, knob_type FROM posteriors WHERE character = ?1")
            .context("Failed to prepare list query")?;
        let rows = stmt
            .query_map(params![character], |row| {
                let knob: String = row.get(0)?;
                let knob_type: String = row.get(1)?;
                Ok((knob, knob_type))
            })
            .context("Failed to query posteriors")?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.context("Failed to read posterior row")?);
        }
        Ok(result)
    }
}

// Accessor fields for Gaussian (used in store.rs)
impl Gaussian {
    pub(crate) fn sum_obs(&self) -> f64 {
        self.sum_obs
    }
    pub(crate) fn count_obs(&self) -> f64 {
        self.count_obs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_and_load_beta_binomial() -> Result<()> {
        let store = PosteriorStore::open_memory()?;
        let mut posterior = BetaBinomial::with_prior(10.0, 5.0);

        store.save_beta_binomial("test_char", "heal_landing", &posterior)?;
        let loaded = store.load_beta_binomial("test_char", "heal_landing")?;

        assert_eq!(loaded.alpha(), 10.0);
        assert_eq!(loaded.beta(), 5.0);
        Ok(())
    }

    #[test]
    fn store_and_load_gaussian() -> Result<()> {
        let store = PosteriorStore::open_memory()?;
        let posterior = Gaussian::with_prior(50.0, 25.0, 1.0);

        store.save_gaussian("test_char", "mana_pct", &posterior)?;
        let loaded = store.load_gaussian("test_char", "mana_pct", 1.0)?;

        assert!((loaded.mean() - 50.0).abs() < 0.01);
        assert!((loaded.variance - 25.0).abs() < 0.01);
        Ok(())
    }

    #[test]
    fn list_knobs_returns_character_posteriors() -> Result<()> {
        let store = PosteriorStore::open_memory()?;
        let p1 = BetaBinomial::new();
        let p2 = Gaussian::new();

        store.save_beta_binomial("char1", "knob_a", &p1)?;
        store.save_gaussian("char1", "knob_b", &p2)?;
        store.save_beta_binomial("char2", "knob_c", &p1)?;

        let char1_knobs = store.list_knobs("char1")?;
        assert_eq!(char1_knobs.len(), 2);
        assert!(char1_knobs.iter().any(|(k, _)| k == "knob_a"));
        assert!(char1_knobs.iter().any(|(k, _)| k == "knob_b"));

        let char2_knobs = store.list_knobs("char2")?;
        assert_eq!(char2_knobs.len(), 1);
        Ok(())
    }
}
