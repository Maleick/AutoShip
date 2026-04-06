//! Hook management -- hardware breakpoint hooks via VEH (DR0-DR3).

pub mod casting;
pub mod dx11_null;
pub mod fingerprint;
pub mod game_loop;
pub mod hwbp;
pub mod movement;
pub mod packet_hook;
pub mod render;
pub mod targeting;

pub fn install_all() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Installing additional hooks...");
    tracing::info!("Additional hook setup complete");
    Ok(())
}

pub fn remove_all() {
    tracing::info!("Removing all hooks...");
    hwbp::remove_all();
    fingerprint::remove();
    tracing::info!("All hooks removed");
}
