//! IPC between the DMFT orchestrator and injected DLLs.
//!
//! - `shared`: reads game state from shared memory (published by DLL)
//! - `pipe`: sends commands to DLL via named pipes, receives responses

pub mod pipe;
pub mod shared;

pub use dmft_common::ipc::{
    load_session_token, session_id_from_token, write_session_token_file,
};
