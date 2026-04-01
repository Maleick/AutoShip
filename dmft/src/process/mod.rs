//! OS-level process interaction — open, read memory, find processes and windows.

/// Process memory reading — `ReadProcessMemory` wrapper and process handle management.
pub mod memory;
/// Window enumeration — finds EQ windows by process ID or title.
pub mod window;
