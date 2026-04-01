//! DLL injection and staging — prepares and loads the DMFT DLL into EQ clients.

/// DLL preparation — copies, renames, and stages the DLL for injection.
pub mod dll_prep;
/// DLL loader — injects the staged DLL into a target EQ process.
pub mod loader;
