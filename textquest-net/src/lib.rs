//! Cross-client communication transports for TextQuest.
//!
//! Provides native equivalents and interop shims for the four MQ2 cross-client
//! transports: EQBC, NetBots, DanNet, and NetMQ.
//!
//! # Transport selection
//!
//! The operator picks one primary transport via [`TransportConfig`]. EQBC and
//! DanNet can run concurrently for rgmercs interop. NetMQ is deferred (stub
//! only) — no live demand found at time of implementation.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────┐
//! │  TextQuest instance A              instance B        │
//! │  ┌───────────┐ vitals  ┌──────┐   ┌──────────────┐  │
//! │  │ NetBots   │────────▶│ hub  │──▶│  NetBots rx  │  │
//! │  │ publisher │         │      │   └──────────────┘  │
//! │  └───────────┘         │EQBC  │                      │
//! │  ┌───────────┐ /bc     │server│   ┌──────────────┐  │
//! │  │ EQBC      │────────▶│      │──▶│  EQBC client │  │
//! │  │ client    │         └──────┘   └──────────────┘  │
//! │  └───────────┘                                       │
//! │  ┌───────────┐ observe ┌──────┐   ┌──────────────┐  │
//! │  │ DanNet    │◀───────▶│ peer │◀─▶│  DanNet peer │  │
//! │  │ node      │         └──────┘   └──────────────┘  │
//! │  └───────────┘                                       │
//! └──────────────────────────────────────────────────────┘
//! ```

pub mod config;
pub mod dannet;
pub mod eqbc;
pub mod netbots;
pub mod netmq;

pub use config::{DanNetConfig, EqbcConfig, NetBotsConfig, TransportConfig, TransportKind};
