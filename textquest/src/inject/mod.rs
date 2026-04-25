//! DLL injection and staging — prepares and loads the TextQuest DLL into EQ
//! clients.

/// DLL preparation — copies, renames, and stages the DLL for injection.
pub mod dll_prep;
/// DLL loader — injects the staged DLL into a target EQ process.
pub mod loader;
/// Polymorphic & self-mutating injection shellcode (EPIC #734).
///
/// Replaces the static reflective loader with per-injection unique loader
/// stubs plus a self-erasing runtime. See [`polymorphic::PolymorphicLoader`].
pub mod polymorphic;
/// Reflective DLL loader — maps a DLL from bytes without touching the
/// filesystem.
pub mod reflective;
