//! Hook management — registers and removes function detours.

pub mod game_loop;
pub mod movement;
pub mod casting;
pub mod targeting;

/// Installs all hooks. Called during DLL initialization.
pub fn install_all() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Installing hooks...");
    // TODO: Install each hook category
    tracing::info!("All hooks installed");
    Ok(())
}

/// Removes all hooks. Called during DLL shutdown.
pub fn remove_all() {
    tracing::info!("Removing all hooks...");
    // TODO: Remove each hook category
    tracing::info!("All hooks removed");
}
