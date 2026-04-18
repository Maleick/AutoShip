//! TextQuest Rust Client Library
//!
//! A Rust client library for interacting with the TextQuest web API.
//! Provides both synchronous and asynchronous clients for REST and WebSocket communication.
//!
//! # Usage
//!
//! ```rust
//! use textquest_client::{Client, ClientConfig};
//!
//! let client = Client::new(ClientConfig::default());
//! let sessions = client.list_sessions().await?;
//! ```
//!
//! # Features
//!
//! - `sync` (default): Synchronous HTTP client
//! - `async`: Asynchronous HTTP client with WebSocket support

pub mod error;
pub mod models;
#[cfg(feature = "sync")]
pub mod sync;
#[cfg(feature = "async")]
pub mod async_client;
#[cfg(feature = "async")]
pub mod websocket;

pub use error::{Error, Result};
pub use models::*;