//! TextQuest Web API Rust SDK
//!
//! A clean async Rust interface for the currently supported TextQuest web API endpoints.
//! Supports both async (tokio) and sync usage via `tokio::runtime`.
//!
//! # Example
//!
//! ```ignore
//! use textquest_web_sdk::Client;
//!
//! #[tokio::main]
//! async fn main() {
//!     let client = Client::new("http://localhost:3000");
//!
//!     // Get health status
//!     match client.health().await {
//!         Ok(health) => println!("Status: {}", health.status),
//!         Err(e) => eprintln!("Error: {}", e),
//!     }
//!
//!     // List sessions
//!     if let Ok(sessions) = client.list_sessions().await {
//!         for session in sessions {
//!             println!("Session: {:?}", session);
//!         }
//!     }
//! }
//! ```

pub mod client;
pub mod error;
pub mod models;
pub mod sync;

pub use client::Client;
pub use error::{Error, Result};
pub use models::*;
pub use sync::BlockingClient;

/// Current version of the TextQuest Web SDK
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
