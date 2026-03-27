//! IPC between the DMFT orchestrator and injected DLLs.
//!
//! - `shared`: reads game state from shared memory (published by DLL)
//! - `pipe`: sends commands to DLL via named pipes, receives responses

pub mod pipe;
pub mod shared;
