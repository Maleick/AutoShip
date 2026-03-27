//! IPC between the injected DLL and the DMFT orchestrator.

pub mod pipe;
pub mod shared;

/// Start IPC listener (named pipe server + shared memory writer).
pub fn start() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Starting IPC...");
    // TODO: Create shared memory region
    // TODO: Start named pipe server thread
    Ok(())
}

/// Stop IPC and clean up.
pub fn stop() {
    tracing::info!("Stopping IPC...");
    // TODO: Close shared memory
    // TODO: Close named pipe
}
