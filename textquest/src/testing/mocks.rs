//! Mock implementations for EQ process reading.

#[cfg(windows)]
use crate::process::memory::ProcessHandle;
use std::collections::HashMap;

/// Trait for reading EQ process memory.
/// Implementors can provide real process reading (Windows) or mocks (tests).
pub trait EqProcessReader {
    fn read<T: Copy + Default>(&mut self, address: usize) -> Result<T, String>;
    fn read_string(&mut self, address: usize, max_len: usize) -> Result<String, String>;
    fn read_ptr(&mut self, address: usize) -> Result<usize, String>;
}

/// Lifecycle state for mock process handles used by macOS integration tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum MockProcessState {
    /// Allocated but not started.
    #[default]
    Created,
    /// Actively running.
    Running,
    /// Stopped by test control flow.
    Stopped,
}


/// Trait for test-process lifecycles.
///
/// This trait provides a small, platform-agnostic abstraction for spawning and
/// controlling fake process objects in test scenarios on any OS.
pub trait MockProcessLifecycle {
    /// Stable PID used by callers.
    fn pid(&self) -> u32;
    /// Human-readable process name.
    fn process_name(&self) -> &str;
    /// Current lifecycle state.
    fn state(&self) -> MockProcessState;

    /// Start the mock process if it is not already running.
    fn start(&mut self);
    /// Stop the mock process if it is currently running.
    fn stop(&mut self);

    /// Convenience check for [`MockProcessState::Running`].
    fn is_running(&self) -> bool {
        matches!(self.state(), MockProcessState::Running)
    }
}

/// In-memory process stub with deterministic lifecycle for macOS-safe tests.
#[derive(Debug, Clone)]
pub struct MockProcess {
    pid: u32,
    process_name: String,
    state: MockProcessState,
}

impl MockProcess {
    /// Create a new mock process handle.
    #[must_use]
    pub fn new(pid: u32) -> Self {
        Self {
            pid,
            process_name: "eqgame.exe".to_string(),
            state: MockProcessState::Created,
        }
    }

    /// Build from a custom executable-style name.
    #[must_use]
    pub fn with_name(mut self, process_name: impl Into<String>) -> Self {
        self.process_name = process_name.into();
        self
    }
}

impl MockProcessLifecycle for MockProcess {
    fn pid(&self) -> u32 {
        self.pid
    }

    fn process_name(&self) -> &str {
        &self.process_name
    }

    fn state(&self) -> MockProcessState {
        self.state
    }

    fn start(&mut self) {
        if matches!(self.state, MockProcessState::Running) {
            return;
        }
        self.state = MockProcessState::Running;
    }

    fn stop(&mut self) {
        if matches!(self.state, MockProcessState::Stopped) {
            return;
        }
        self.state = MockProcessState::Stopped;
    }
}

/// Mock implementation for testing.
/// Stores pre-configured values that are returned on reads.
pub struct MockProcessReader {
    pid: u32,
    memory: HashMap<usize, Vec<u8>>,
    base: u64,
}

impl MockProcessReader {
    pub fn new(pid: u32) -> Self {
        Self {
            pid,
            memory: HashMap::new(),
            base: 0x140000000,
        }
    }

    pub fn with_base(mut self, base: u64) -> Self {
        self.base = base;
        self
    }

    pub fn with_value<T: bytemuck::Pod>(mut self, address: usize, value: T) -> Self {
        let bytes = bytemuck::bytes_of(&value).to_vec();
        self.memory.insert(address, bytes);
        self
    }

    pub fn with_string(mut self, address: usize, value: &str) -> Self {
        let mut bytes = value.as_bytes().to_vec();
        bytes.push(0);
        self.memory.insert(address, bytes);
        self
    }

    pub fn with_wide_string(mut self, address: usize, value: &str) -> Self {
        let bytes: Vec<u8> = value
            .encode_utf16()
            .flat_map(|unit| unit.to_le_bytes())
            .collect();
        self.memory.insert(address, bytes);
        self
    }

    fn memory_at(&self, address: usize) -> Option<&[u8]> {
        self.memory.get(&address).map(Vec::as_slice)
    }

    fn decode_value<T: Copy + Default>(&self, address: usize) -> Result<T, String> {
        let Some(bytes) = self.memory_at(address) else {
            return Ok(T::default());
        };

        let size = std::mem::size_of::<T>();
        if bytes.len() < size {
            return Err(format!(
                "configured value at 0x{address:X} is too short: expected at least {size} bytes, got {}",
                bytes.len()
            ));
        }

        let mut value = std::mem::MaybeUninit::<T>::uninit();
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), value.as_mut_ptr() as *mut u8, size);
            Ok(value.assume_init())
        }
    }

    fn decode_string(&self, address: usize, max_len: usize) -> Result<String, String> {
        let Some(bytes) = self.memory_at(address) else {
            return Ok(String::new());
        };

        let truncated = &bytes[..bytes.len().min(max_len)];

        if truncated.len() >= 2
            && truncated.len() % 2 == 0
            && truncated.iter().skip(1).step_by(2).any(|&b| b == 0)
        {
            let units: Vec<u16> = truncated
                .chunks_exact(2)
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .take_while(|&unit| unit != 0)
                .collect();
            return String::from_utf16(&units).map_err(|e| e.to_string());
        }

        let end = truncated
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(truncated.len());
        Ok(String::from_utf8_lossy(&truncated[..end]).into_owned())
    }

    fn decode_ptr(&self, address: usize) -> Result<usize, String> {
        let Some(bytes) = self.memory_at(address) else {
            return Ok(0);
        };

        let size = std::mem::size_of::<usize>();
        if bytes.len() < size {
            return Err(format!(
                "configured pointer at 0x{address:X} is too short: expected at least {size} bytes, got {}",
                bytes.len()
            ));
        }

        let mut raw = [0u8; std::mem::size_of::<usize>()];
        raw.copy_from_slice(&bytes[..size]);
        Ok(usize::from_ne_bytes(raw))
    }
}

impl EqProcessReader for MockProcessReader {
    fn read<T: Copy + Default>(&mut self, address: usize) -> Result<T, String> {
        self.decode_value(address)
    }

    fn read_string(&mut self, address: usize, max_len: usize) -> Result<String, String> {
        self.decode_string(address, max_len)
    }

    fn read_ptr(&mut self, address: usize) -> Result<usize, String> {
        self.decode_ptr(address)
    }
}

/// Real process reader for Windows.
/// Wraps ProcessHandle to read actual EQ process memory.
#[cfg(windows)]
pub struct RealProcessReader {
    handle: ProcessHandle,
}

#[cfg(windows)]
impl RealProcessReader {
    pub fn new(pid: u32) -> Result<Self, String> {
        Ok(Self {
            handle: ProcessHandle::open(pid).map_err(|e| e.to_string())?,
        })
    }
}

#[cfg(windows)]
impl EqProcessReader for RealProcessReader {
    fn read<T: Copy + Default>(&mut self, address: usize) -> Result<T, String> {
        self.handle.read(address).map_err(|e| e.to_string())
    }

    fn read_string(&mut self, address: usize, max_len: usize) -> Result<String, String> {
        self.handle
            .read_string(address, max_len)
            .map_err(|e| e.to_string())
    }

    fn read_ptr(&mut self, address: usize) -> Result<usize, String> {
        self.handle.read_ptr(address).map_err(|e| e.to_string())
    }
}

/// Non-Windows stub for RealProcessReader.
/// Always fails since we cannot read actual process memory on non-Windows platforms.
#[cfg(not(windows))]
pub struct RealProcessReader {
    pid: u32,
}

#[cfg(not(windows))]
impl RealProcessReader {
    pub fn new(pid: u32) -> Result<Self, String> {
        tracing::trace!(
            pid,
            "RealProcessReader::new called on non-Windows platform (stub)"
        );
        Ok(Self { pid })
    }
}

#[cfg(not(windows))]
impl EqProcessReader for RealProcessReader {
    fn read<T: Copy + Default>(&mut self, _address: usize) -> Result<T, String> {
        Err(format!(
            "Cannot read process memory on non-Windows platform (pid={})",
            self.pid
        ))
    }

    fn read_string(&mut self, _address: usize, _max_len: usize) -> Result<String, String> {
        Err(format!(
            "Cannot read process memory on non-Windows platform (pid={})",
            self.pid
        ))
    }

    fn read_ptr(&mut self, _address: usize) -> Result<usize, String> {
        Err(format!(
            "Cannot read process memory on non-Windows platform (pid={})",
            self.pid
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_reader_returns_configured_values() {
        let mut mock = MockProcessReader::new(1234).with_value(0x140000000, 0xCAFEBABEu32);

        let value: u32 = mock.read(0x140000000).unwrap();
        assert_eq!(value, 0xCAFEBABE);
    }

    #[test]
    fn mock_reader_returns_configured_string() {
        let mut mock = MockProcessReader::new(1234).with_string(0x140000000, "TestChar");

        let value = mock.read_string(0x140000000, 32).unwrap();
        assert_eq!(value, "TestChar");
    }

    #[test]
    fn mock_reader_returns_default_for_unconfigured_address() {
        let mut mock = MockProcessReader::new(1234);

        let value: u32 = mock.read(0x99999999).unwrap();
        assert_eq!(value, 0);
    }

    #[test]
    fn real_process_reader_can_be_created() {
        // On non-Windows platforms, construction succeeds but actual reads fail.
        // On Windows platforms, construction succeeds if the PID is valid.
        let result = RealProcessReader::new(9999);
        // Just verify it can be created; actual success depends on platform and PID.
        let _ = result;
    }

    #[cfg(not(windows))]
    #[test]
    fn real_process_reader_stub_fails_on_read() {
        let mut reader = RealProcessReader::new(1234).expect("stub should construct");
        let result: Result<u32, String> = reader.read(0x140000000);
        assert!(result.is_err(), "non-Windows stub should fail reads");
    }

    #[cfg(not(windows))]
    #[test]
    fn real_process_reader_stub_fails_on_read_string() {
        let mut reader = RealProcessReader::new(1234).expect("stub should construct");
        let result = reader.read_string(0x140000000, 32);
        assert!(result.is_err(), "non-Windows stub should fail string reads");
    }

    #[cfg(not(windows))]
    #[test]
    fn real_process_reader_stub_fails_on_read_ptr() {
        let mut reader = RealProcessReader::new(1234).expect("stub should construct");
        let result = reader.read_ptr(0x140000000);
        assert!(
            result.is_err(),
            "non-Windows stub should fail pointer reads"
        );
    }
}
