//! NetMQ transport stub.
//!
//! # Status: Deferred
//!
//! MQ2NetMQ is a ZeroMQ-based transport that appears in some MQ2 setups but
//! is far less prevalent than EQBC and DanNet.  No live demand was found
//! during the issue-2612 design pass.
//!
//! This module exists as an explicit record of the decision and as a compile-
//! time anchor point for future work.  It exports a no-op placeholder so that
//! the rest of the codebase can reference `netmq::NetMqTransport` without
//! conditional compilation noise.
//!
//! ## Rationale for deferral
//!
//! 1. ZeroMQ introduces a non-trivial native dependency (libzmq / zeromq crate)
//!    that complicates Windows cross-compilation.
//! 2. EQBC and DanNet cover 99%+ of active multibox setups (Teek TLP intel,
//!    rgmercs docs).
//! 3. The ZeroMQ framing layer can be added later on top of the same
//!    `TransportConfig::kind` enum without breaking existing users.
//!
//! ## What to do when demand arises
//!
//! 1. Add `zeromq = "0.4"` to `textquest-net/Cargo.toml`.
//! 2. Implement `NetMqTransport::bind` / `connect` / `publish` / `subscribe`.
//! 3. Add `TransportKind::NetMq` to `config.rs`.
//! 4. Route through the orchestrator startup path the same way EQBC/DanNet do.

/// Stub transport — all methods are no-ops that log a warning.
#[derive(Debug, Default)]
pub struct NetMqTransport;

impl NetMqTransport {
    /// Attempt to start the NetMQ transport.  Always returns an error because
    /// NetMQ is not yet implemented.
    ///
    /// # Errors
    ///
    /// Always returns `Err` — NetMQ transport is deferred pending demand.
    pub fn start(&self) -> anyhow::Result<()> {
        anyhow::bail!(
            "NetMQ transport is not implemented. \
             Use TransportKind::Eqbc or TransportKind::DanNet instead."
        )
    }
}
