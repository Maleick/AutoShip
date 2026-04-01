use anyhow::{Context, Result};

/// Prompt the user for a master password without echoing to the terminal.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn prompt_master_password() -> Result<String> {
    rpassword::prompt_password("Master password: ").context("Failed to read master password")
}
