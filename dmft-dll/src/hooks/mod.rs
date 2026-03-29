//! Hook management -- registers and removes function detours.

pub mod casting;
pub mod game_loop;
pub mod movement;
pub mod render;
pub mod targeting;

/// Installs all hooks. Called during DLL initialization.
///
/// Note: The game loop hook is installed separately in `lib.rs::install_hooks()`
/// using the resolved EQ base address + `MAIN_LOOP_OFFSET`. This function
/// handles any additional hooks (casting, targeting, etc.) once they are ready.
pub fn install_all() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Installing additional hooks...");

    // Game loop hook is installed via lib.rs::install_hooks() with the resolved base.
    // Additional hooks (casting, targeting, movement) are function-call APIs,
    // not detours — they don't need install/remove lifecycle.

    tracing::info!("Additional hook setup complete");
    Ok(())
}

/// Removes all hooks. Called during DLL shutdown.
///
/// Note: movement, casting, and targeting are function-call APIs (not
/// detour hooks) so they have no install/remove lifecycle.
pub fn remove_all() {
    tracing::info!("Removing all hooks...");
    game_loop::remove();
    render::remove();
    tracing::info!("All hooks removed");
}
