//! Hook management -- registers and removes function detours.

pub mod casting;
pub mod game_loop;
pub mod movement;
pub mod targeting;

/// Installs all hooks. Called during DLL initialization.
pub fn install_all() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Installing hooks...");

    // TODO: Resolve actual function addresses from EQ base + offsets.
    // For now, skip installation since we don't have real addresses.
    // Example once addresses are known:
    //   let base = get_module_base("eqgame.exe")?;
    //   game_loop::install(base + eq::MAIN_LOOP_OFFSET)?;

    tracing::info!("Hook installation skipped (addresses not yet resolved)");
    Ok(())
}

/// Removes all hooks. Called during DLL shutdown.
pub fn remove_all() {
    tracing::info!("Removing all hooks...");
    game_loop::remove();
    // movement::remove();
    // casting::remove();
    // targeting::remove();
    tracing::info!("All hooks removed");
}
