use anyhow::{Context, Result};
use zeroize::Zeroizing;

/// Prompt the user for a password without echoing to the terminal.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn prompt_password(prompt: &str) -> Result<Zeroizing<String>> {
    let password = rpassword::prompt_password(prompt).context("Failed to read password")?;
    Ok(Zeroizing::new(password))
}

/// Prompt the user for a master password without echoing to the terminal.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn prompt_master_password() -> Result<Zeroizing<String>> {
    prompt_password("Master password: ")
}
