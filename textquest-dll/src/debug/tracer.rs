//! Low-overhead EQ function call tracer backed by HWBP slots.
//!
//! The exception-handler path avoids allocation and locks: it matches the
//! trapped RIP against atomic trace state, copies register values into a static
//! ring buffer, and returns. IPC-facing start/stop/list/dump operations handle
//! address parsing, slot installation, and record formatting off the hot path.

use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering},
};

use textquest_common::ipc::{TraceRecord, TraceStatus};

use crate::hooks::hwbp::{self, HwbpInstallOutcome, HwbpSlot, MAX_SLOTS};

const MAX_TRACES: usize = MAX_SLOTS;
const TRACE_RING_CAPACITY: usize = 4096;
const MAX_TRACE_DUMP_RECORDS: u64 = 1024;
const NO_SLOT: usize = usize::MAX;

const FLAG_ARGS: u32 = 1 << 0;
const FLAG_RETURN_ADDRESS: u32 = 1 << 1;

static TRACE_CONTROL: Mutex<()> = Mutex::new(());
static TRACE_NAMES: OnceLock<Mutex<[Option<String>; MAX_TRACES]>> = OnceLock::new();
static TRACES: [TraceState; MAX_TRACES] = [const { TraceState::new() }; MAX_TRACES];
static TRACE_RING: [TraceEventSlot; TRACE_RING_CAPACITY] =
    [const { TraceEventSlot::new() }; TRACE_RING_CAPACITY];
static EVENT_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static DUMP_CURSOR: AtomicU64 = AtomicU64::new(0);
static TOTAL_DROPPED_EVENTS: AtomicU64 = AtomicU64::new(0);

struct TraceState {
    active: AtomicBool,
    address: AtomicUsize,
    capture_args: AtomicBool,
    capture_return: AtomicBool,
    slot: AtomicUsize,
    call_count: AtomicU64,
}

impl TraceState {
    const fn new() -> Self {
        Self {
            active: AtomicBool::new(false),
            address: AtomicUsize::new(0),
            capture_args: AtomicBool::new(false),
            capture_return: AtomicBool::new(false),
            slot: AtomicUsize::new(NO_SLOT),
            call_count: AtomicU64::new(0),
        }
    }

    fn configure(&self, address: usize, capture_args: bool, capture_return: bool) {
        self.call_count.store(0, Ordering::Release);
        self.slot.store(NO_SLOT, Ordering::Release);
        self.address.store(address, Ordering::Release);
        self.capture_args.store(capture_args, Ordering::Release);
        self.capture_return.store(capture_return, Ordering::Release);
        self.active.store(true, Ordering::Release);
    }

    fn clear(&self) {
        self.active.store(false, Ordering::Release);
        self.address.store(0, Ordering::Release);
        self.capture_args.store(false, Ordering::Release);
        self.capture_return.store(false, Ordering::Release);
        self.slot.store(NO_SLOT, Ordering::Release);
        self.call_count.store(0, Ordering::Release);
    }
}

struct TraceEventSlot {
    version: AtomicU64,
    sequence: AtomicU64,
    trace_index: AtomicUsize,
    address: AtomicUsize,
    timestamp_ticks: AtomicU64,
    thread_id: AtomicU32,
    flags: AtomicU32,
    arg0: AtomicU64,
    arg1: AtomicU64,
    arg2: AtomicU64,
    arg3: AtomicU64,
    stack_pointer: AtomicU64,
    return_address: AtomicU64,
}

impl TraceEventSlot {
    const fn new() -> Self {
        Self {
            version: AtomicU64::new(0),
            sequence: AtomicU64::new(0),
            trace_index: AtomicUsize::new(0),
            address: AtomicUsize::new(0),
            timestamp_ticks: AtomicU64::new(0),
            thread_id: AtomicU32::new(0),
            flags: AtomicU32::new(0),
            arg0: AtomicU64::new(0),
            arg1: AtomicU64::new(0),
            arg2: AtomicU64::new(0),
            arg3: AtomicU64::new(0),
            stack_pointer: AtomicU64::new(0),
            return_address: AtomicU64::new(0),
        }
    }
}

#[derive(Clone, Copy)]
struct HitSample {
    address: usize,
    timestamp_ticks: u64,
    thread_id: u32,
    args: [u64; 4],
    stack_pointer: u64,
    return_address: u64,
}

struct RawTraceRecord {
    trace_index: usize,
    address: usize,
    sequence: u64,
    timestamp_ticks: u64,
    thread_id: u32,
    flags: u32,
    args: [u64; 4],
    stack_pointer: u64,
    return_address: u64,
}

/// Start tracing a function using an available HWBP slot.
pub fn start(
    function_name: String,
    capture_args: bool,
    capture_return: bool,
) -> Result<TraceStatus, String> {
    let address = parse_trace_address(&function_name).ok_or_else(|| {
        format!(
            "TraceStart requires an absolute address, or a label with @0x... suffix: {function_name}"
        )
    })?;
    if address == 0 {
        return Err("TraceStart address must be non-zero".into());
    }

    let _guard = TRACE_CONTROL
        .lock()
        .map_err(|_| "trace control mutex poisoned".to_string())?;

    if let Some(existing) = find_trace_index(&function_name, Some(address)) {
        return status_for(existing)
            .ok_or_else(|| "trace was active but could not build status".to_string());
    }

    let trace_index = TRACES
        .iter()
        .position(|trace| !trace.active.load(Ordering::Acquire))
        .ok_or_else(|| "all trace descriptors are in use".to_string())?;

    names_lock()?[trace_index] = Some(function_name);
    TRACES[trace_index].configure(address, capture_args, capture_return);

    match hwbp::register_available(address, trace_entry_callback) {
        Ok(HwbpInstallOutcome::Installed(slot)) => {
            TRACES[trace_index]
                .slot
                .store(slot as usize, Ordering::Release);
            status_for(trace_index)
                .ok_or_else(|| "trace installed but could not build status".to_string())
        }
        Ok(HwbpInstallOutcome::FallbackToDetour) => {
            clear_trace(trace_index);
            Err("no hardware breakpoint slots available for trace".into())
        }
        Err(error) => {
            clear_trace(trace_index);
            Err(format!("failed to install trace HWBP: {error}"))
        }
    }
}

/// Stop tracing a function by its original label or address.
pub fn stop(function_name: &str) -> Result<TraceStatus, String> {
    let address = parse_trace_address(function_name);
    let _guard = TRACE_CONTROL
        .lock()
        .map_err(|_| "trace control mutex poisoned".to_string())?;
    let trace_index = find_trace_index(function_name, address)
        .ok_or_else(|| format!("trace is not active: {function_name}"))?;
    let status =
        status_for(trace_index).ok_or_else(|| format!("trace is not active: {function_name}"))?;
    let slot = TRACES[trace_index].slot.load(Ordering::Acquire);

    if let Some(slot) = HwbpSlot::from_index(slot) {
        hwbp::unregister(slot)
            .map_err(|error| format!("failed to unregister trace HWBP: {error}"))?;
    }
    clear_trace(trace_index);

    Ok(status)
}

/// List active traces and counters.
pub fn list() -> Vec<TraceStatus> {
    (0..MAX_TRACES).filter_map(status_for).collect()
}

/// Drain trace events captured since the previous dump.
pub fn dump() -> (Vec<TraceRecord>, u64) {
    let end = EVENT_SEQUENCE.load(Ordering::Acquire);
    let previous = DUMP_CURSOR.swap(end, Ordering::AcqRel).min(end);
    let capacity = TRACE_RING_CAPACITY as u64;
    let ring_overflow_dropped = end.saturating_sub(previous).saturating_sub(capacity);
    let start = if ring_overflow_dropped > 0 {
        end.saturating_sub(capacity)
    } else {
        previous
    };
    let available = end.saturating_sub(start);
    let dump_cap = MAX_TRACE_DUMP_RECORDS.min(capacity);
    let dump_trimmed_dropped = available.saturating_sub(dump_cap);
    let dropped = ring_overflow_dropped.saturating_add(dump_trimmed_dropped);
    let effective_start = if dump_trimmed_dropped > 0 {
        end.saturating_sub(dump_cap)
    } else {
        start
    };
    if dropped > 0 {
        TOTAL_DROPPED_EVENTS.fetch_add(dropped, Ordering::Relaxed);
    }

    let mut records =
        Vec::with_capacity(end.saturating_sub(effective_start).min(dump_cap) as usize);
    for sequence in (effective_start + 1)..=end {
        if let Some(raw) = read_ring_record(sequence) {
            let name = trace_name(raw.trace_index);
            let args = if raw.flags & FLAG_ARGS != 0 {
                Some(raw.args)
            } else {
                None
            };
            records.push(TraceRecord {
                function_name: name,
                address: raw.address,
                sequence: raw.sequence,
                timestamp_ticks: raw.timestamp_ticks,
                thread_id: raw.thread_id,
                args,
                stack_pointer: if raw.flags & FLAG_ARGS != 0 {
                    Some(raw.stack_pointer)
                } else {
                    None
                },
                return_address: if raw.flags & FLAG_RETURN_ADDRESS != 0 {
                    Some(raw.return_address)
                } else {
                    None
                },
                return_value: None,
                elapsed_ticks: None,
            });
        }
    }

    (records, dropped)
}

fn trace_entry_callback(exception_info: *mut ()) -> bool {
    let Some(sample) = capture_hit_sample(exception_info) else {
        return false;
    };

    for (trace_index, trace) in TRACES.iter().enumerate() {
        if !trace.active.load(Ordering::Acquire) {
            continue;
        }
        if trace.address.load(Ordering::Acquire) != sample.address {
            continue;
        }

        trace.call_count.fetch_add(1, Ordering::Relaxed);
        record_hit(
            trace_index,
            sample,
            trace.capture_args.load(Ordering::Acquire),
            trace.capture_return.load(Ordering::Acquire),
        );
        return true;
    }

    false
}

fn record_hit(trace_index: usize, sample: HitSample, capture_args: bool, capture_return: bool) {
    let sequence = EVENT_SEQUENCE.fetch_add(1, Ordering::Relaxed) + 1;
    let slot = &TRACE_RING[((sequence - 1) as usize) % TRACE_RING_CAPACITY];
    let version = sequence.saturating_mul(2);
    let mut flags = 0;
    if capture_args {
        flags |= FLAG_ARGS;
    }
    if capture_return {
        flags |= FLAG_RETURN_ADDRESS;
    }

    slot.version.store(version | 1, Ordering::Release);
    slot.sequence.store(sequence, Ordering::Relaxed);
    slot.trace_index.store(trace_index, Ordering::Relaxed);
    slot.address.store(sample.address, Ordering::Relaxed);
    slot.timestamp_ticks
        .store(sample.timestamp_ticks, Ordering::Relaxed);
    slot.thread_id.store(sample.thread_id, Ordering::Relaxed);
    slot.flags.store(flags, Ordering::Relaxed);
    slot.arg0.store(sample.args[0], Ordering::Relaxed);
    slot.arg1.store(sample.args[1], Ordering::Relaxed);
    slot.arg2.store(sample.args[2], Ordering::Relaxed);
    slot.arg3.store(sample.args[3], Ordering::Relaxed);
    slot.stack_pointer
        .store(sample.stack_pointer, Ordering::Relaxed);
    slot.return_address
        .store(sample.return_address, Ordering::Relaxed);
    slot.version.store(version, Ordering::Release);
}

fn read_ring_record(expected_sequence: u64) -> Option<RawTraceRecord> {
    let slot = &TRACE_RING[((expected_sequence - 1) as usize) % TRACE_RING_CAPACITY];
    for _ in 0..2 {
        let before = slot.version.load(Ordering::Acquire);
        if before & 1 != 0 {
            continue;
        }
        let sequence = slot.sequence.load(Ordering::Relaxed);
        let raw = RawTraceRecord {
            trace_index: slot.trace_index.load(Ordering::Relaxed),
            address: slot.address.load(Ordering::Relaxed),
            sequence,
            timestamp_ticks: slot.timestamp_ticks.load(Ordering::Relaxed),
            thread_id: slot.thread_id.load(Ordering::Relaxed),
            flags: slot.flags.load(Ordering::Relaxed),
            args: [
                slot.arg0.load(Ordering::Relaxed),
                slot.arg1.load(Ordering::Relaxed),
                slot.arg2.load(Ordering::Relaxed),
                slot.arg3.load(Ordering::Relaxed),
            ],
            stack_pointer: slot.stack_pointer.load(Ordering::Relaxed),
            return_address: slot.return_address.load(Ordering::Relaxed),
        };
        let after = slot.version.load(Ordering::Acquire);
        if before == after && sequence == expected_sequence {
            return Some(raw);
        }
    }
    None
}

#[cfg(windows)]
fn capture_hit_sample(exception_info: *mut ()) -> Option<HitSample> {
    use windows::Win32::System::{
        Diagnostics::Debug::EXCEPTION_POINTERS, Threading::GetCurrentThreadId,
    };

    if exception_info.is_null() {
        return None;
    }
    let info = unsafe { &*(exception_info as *const EXCEPTION_POINTERS) };
    let context = unsafe { &*info.ContextRecord };
    let return_address = if context.Rsp == 0 {
        0
    } else {
        unsafe { std::ptr::read_unaligned(context.Rsp as *const u64) }
    };

    Some(HitSample {
        address: context.Rip as usize,
        timestamp_ticks: timestamp_ticks(),
        thread_id: unsafe { GetCurrentThreadId() },
        args: [context.Rcx, context.Rdx, context.R8, context.R9],
        stack_pointer: context.Rsp,
        return_address,
    })
}

#[cfg(not(windows))]
fn capture_hit_sample(_exception_info: *mut ()) -> Option<HitSample> {
    None
}

fn timestamp_ticks() -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        unsafe { core::arch::x86_64::_rdtsc() }
    }
    #[cfg(target_arch = "x86")]
    {
        unsafe { core::arch::x86::_rdtsc() as u64 }
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
    {
        0
    }
}

fn parse_trace_address(input: &str) -> Option<usize> {
    let trimmed = input.trim();
    let candidate = trimmed
        .rsplit_once('@')
        .or_else(|| trimmed.rsplit_once('='))
        .map_or(trimmed, |(_, value)| value.trim());
    parse_numeric_address(candidate)
}

fn parse_numeric_address(input: &str) -> Option<usize> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    let hex = input
        .strip_prefix("0x")
        .or_else(|| input.strip_prefix("0X"));
    if let Some(hex) = hex {
        return usize::from_str_radix(hex, 16).ok();
    }
    input.parse::<usize>().ok()
}

fn names_lock() -> Result<std::sync::MutexGuard<'static, [Option<String>; MAX_TRACES]>, String> {
    TRACE_NAMES
        .get_or_init(|| Mutex::new(std::array::from_fn(|_| None)))
        .lock()
        .map_err(|_| "trace names mutex poisoned".to_string())
}

fn trace_name(trace_index: usize) -> String {
    names_lock()
        .ok()
        .and_then(|names| names.get(trace_index).and_then(Clone::clone))
        .unwrap_or_else(|| format!("trace#{trace_index}"))
}

fn find_trace_index(function_name: &str, address: Option<usize>) -> Option<usize> {
    let names = names_lock().ok()?;
    (0..MAX_TRACES).find(|index| {
        if !TRACES[*index].active.load(Ordering::Acquire) {
            return false;
        }
        if names[*index].as_deref() == Some(function_name) {
            return true;
        }
        address.is_some_and(|address| TRACES[*index].address.load(Ordering::Acquire) == address)
    })
}

fn status_for(trace_index: usize) -> Option<TraceStatus> {
    let trace = &TRACES[trace_index];
    if !trace.active.load(Ordering::Acquire) {
        return None;
    }
    let slot = trace.slot.load(Ordering::Acquire);
    Some(TraceStatus {
        function_name: trace_name(trace_index),
        address: trace.address.load(Ordering::Acquire),
        capture_args: trace.capture_args.load(Ordering::Acquire),
        capture_return: trace.capture_return.load(Ordering::Acquire),
        slot: (slot != NO_SLOT).then_some(slot as u8),
        call_count: trace.call_count.load(Ordering::Acquire),
        dropped_events: TOTAL_DROPPED_EVENTS.load(Ordering::Relaxed),
    })
}

fn clear_trace(trace_index: usize) {
    if let Ok(mut names) = names_lock() {
        names[trace_index] = None;
    }
    TRACES[trace_index].clear();
}

#[cfg(test)]
fn reset_for_test() {
    let _guard = TRACE_CONTROL.lock().expect("trace mutex poisoned");
    for index in 0..MAX_TRACES {
        TRACES[index].clear();
        if let Ok(mut names) = names_lock() {
            names[index] = None;
        }
    }
    EVENT_SEQUENCE.store(0, Ordering::Release);
    DUMP_CURSOR.store(0, Ordering::Release);
    TOTAL_DROPPED_EVENTS.store(0, Ordering::Release);
}

#[cfg(test)]
fn configure_trace_for_test(index: usize, name: &str, address: usize) {
    names_lock().expect("trace names mutex poisoned")[index] = Some(name.to_string());
    TRACES[index].configure(address, true, true);
    TRACES[index].slot.store(index, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_address_variants() {
        assert_eq!(parse_trace_address("0x1234"), Some(0x1234));
        assert_eq!(parse_trace_address("4660"), Some(4660));
        assert_eq!(
            parse_trace_address("CEverQuest::dsp_chat@0x1234"),
            Some(0x1234)
        );
        assert_eq!(parse_trace_address("dsp_chat=0x1234"), Some(0x1234));
        assert_eq!(parse_trace_address("dsp_chat"), None);
    }

    #[test]
    fn dump_flushes_recorded_hits_with_trace_names() {
        reset_for_test();
        configure_trace_for_test(0, "target@0x5000", 0x5000);

        record_hit(
            0,
            HitSample {
                address: 0x5000,
                timestamp_ticks: 42,
                thread_id: 7,
                args: [1, 2, 3, 4],
                stack_pointer: 0x9000,
                return_address: 0x6000,
            },
            true,
            true,
        );

        let (records, dropped) = dump();
        assert_eq!(dropped, 0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].function_name, "target@0x5000");
        assert_eq!(records[0].address, 0x5000);
        assert_eq!(records[0].args, Some([1, 2, 3, 4]));
        assert_eq!(records[0].stack_pointer, Some(0x9000));
        assert_eq!(records[0].return_address, Some(0x6000));

        let (records, dropped) = dump();
        assert_eq!(dropped, 0);
        assert!(records.is_empty());
    }

    #[test]
    fn dump_caps_records_and_counts_trimmed_events_as_dropped() {
        reset_for_test();
        configure_trace_for_test(0, "target@0x5000", 0x5000);

        let total = MAX_TRACE_DUMP_RECORDS as usize + 17;
        for i in 0..total {
            record_hit(
                0,
                HitSample {
                    address: 0x5000,
                    timestamp_ticks: i as u64,
                    thread_id: 7,
                    args: [1, 2, 3, 4],
                    stack_pointer: 0x9000,
                    return_address: 0x6000,
                },
                true,
                true,
            );
        }

        let (records, dropped) = dump();
        assert_eq!(records.len(), MAX_TRACE_DUMP_RECORDS as usize);
        assert_eq!(dropped, 17);
        assert_eq!(records[0].sequence, 18);
        assert_eq!(
            records.last().map(|record| record.sequence),
            Some(total as u64)
        );
    }
}
