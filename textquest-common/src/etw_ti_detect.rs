//! ETW-TI event parser and LoadLibrary injection detector.
//!
//! Parses JSONL output from EtwTiViewer (or compatible ETW consumers) and
//! detects the classic `CreateRemoteThread + LoadLibraryW` DLL injection
//! pattern by correlating three sequential ETW-TI kernel events:
//!
//! 1. `ALLOCVM_REMOTE` (Event ID 1) — `VirtualAllocEx` into a remote process
//! 2. `WRITEVM_REMOTE` (Event ID 2) — `WriteProcessMemory` with the DLL path
//! 3. `IMAGELOAD` (Event ID 5, `Microsoft-Windows-Kernel-Process`) — the DLL
//!    mapping that occurs when `LoadLibraryW` calls `LdrLoadDll`
//!
//! See `docs/etw-ti-loadlibrary.md` for full field documentation and capture
//! instructions.

use std::collections::HashMap;

/// ETW-TI event identifiers for the LoadLibrary injection path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EtwTiEventId {
    /// VirtualAllocEx into a remote process (ETW-TI Event ID 1).
    AllocVmRemote = 1,
    /// WriteProcessMemory into a remote process (ETW-TI Event ID 2).
    WriteVmRemote = 2,
    /// ReadProcessMemory from a remote process (ETW-TI Event ID 5).
    ReadVmRemote = 5,
    /// SetThreadContext on a remote thread (ETW-TI Event ID 10).
    SetThreadContext = 10,
    /// NtMapViewOfSection into remote process (ETW-TI Event ID 12).
    MapViewRemote = 12,
    /// NtQueueApcThread into remote thread (ETW-TI Event ID 14).
    QueueUserApcRemote = 14,
}

impl EtwTiEventId {
    /// Parse from a raw event ID integer.
    pub fn from_id(id: u32) -> Option<Self> {
        match id {
            1 => Some(Self::AllocVmRemote),
            2 => Some(Self::WriteVmRemote),
            5 => Some(Self::ReadVmRemote),
            10 => Some(Self::SetThreadContext),
            12 => Some(Self::MapViewRemote),
            14 => Some(Self::QueueUserApcRemote),
            _ => None,
        }
    }
}

/// A parsed ETW-TI event from JSONL output.
#[derive(Debug, Clone)]
pub struct EtwTiEvent {
    /// Event identifier.
    pub event_id: u32,
    /// ETW provider name (e.g. `"Microsoft-Windows-Threat-Intelligence"`).
    pub provider: String,
    /// Keyword/category name (e.g. `"ALLOCVM_REMOTE"`).
    pub keyword: String,
    /// ISO-8601 timestamp string.
    pub timestamp: String,
    /// PID of the calling process (injector).
    pub calling_process_id: Option<u32>,
    /// Name of the calling process.
    pub calling_process_name: Option<String>,
    /// PID of the target process (injectee).
    pub target_process_id: Option<u32>,
    /// Name of the target process.
    pub target_process_name: Option<String>,
    /// Base address (hex string) for alloc/write events.
    pub base_address: Option<String>,
    /// Byte count written (WriteVmRemote).
    pub byte_count: Option<u64>,
    /// Image path (ImageLoad events from Kernel-Process provider).
    pub image_name: Option<String>,
    /// Raw key-value pairs for fields not explicitly modelled.
    pub extra: HashMap<String, serde_json::Value>,
}

impl EtwTiEvent {
    /// Parse a single JSONL line into an `EtwTiEvent`.
    ///
    /// Returns `None` if the line is not valid JSON or is missing the
    /// required `EventId` field.
    pub fn from_jsonl(line: &str) -> Option<Self> {
        let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
        let obj = v.as_object()?;

        let event_id = obj.get("EventId")?.as_u64()? as u32;
        let provider = obj
            .get("Provider")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let keyword = obj
            .get("Keyword")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let timestamp = obj
            .get("Timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let calling_process_id = obj
            .get("CallingProcessId")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32);
        let calling_process_name = obj
            .get("CallingProcessName")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let target_process_id = obj
            .get("TargetProcessId")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32);
        let target_process_name = obj
            .get("TargetProcessName")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let base_address = obj
            .get("BaseAddress")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let byte_count = obj.get("ByteCount").and_then(|v| v.as_u64());
        let image_name = obj
            .get("ImageName")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Collect remaining fields as extra context.
        let known = [
            "EventId",
            "Provider",
            "Keyword",
            "Timestamp",
            "CallingProcessId",
            "CallingProcessName",
            "TargetProcessId",
            "TargetProcessName",
            "BaseAddress",
            "ByteCount",
            "ImageName",
        ];
        let extra = obj
            .iter()
            .filter(|(k, _)| !known.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        Some(Self {
            event_id,
            provider,
            keyword,
            timestamp,
            calling_process_id,
            calling_process_name,
            target_process_id,
            target_process_name,
            base_address,
            byte_count,
            image_name,
            extra,
        })
    }
}

/// Detection result from [`detect_loadlibrary_injection`].
#[derive(Debug, Clone)]
pub struct InjectionDetection {
    /// PID of the process performing the injection.
    pub injector_pid: u32,
    /// Name of the injecting process.
    pub injector_name: Option<String>,
    /// PID of the process being injected into.
    pub target_pid: u32,
    /// Name of the target process.
    pub target_name: Option<String>,
    /// Base address allocated for the DLL path string.
    pub alloc_address: Option<String>,
    /// Image path of the loaded DLL (if ImageLoad event was captured).
    pub image_name: Option<String>,
    /// Timestamp of the first event (AllocVmRemote).
    pub first_seen: String,
}

/// Detect `CreateRemoteThread + LoadLibraryW` injection from a slice of parsed
/// ETW-TI events.
///
/// The detector looks for this three-event sequence within the event stream:
///
/// 1. `ALLOCVM_REMOTE` where `CallingProcessId != TargetProcessId`
/// 2. `WRITEVM_REMOTE` from the same caller to the same target at the same base
///    address
/// 3. (Optional) `IMAGELOAD` in the target process
///
/// Events need not be adjacent — other events may interleave — but they must
/// appear in order and share the same `(CallingProcessId, TargetProcessId)`
/// pair.
///
/// Returns a list of detections, one per unique injector→target pair that
/// matches the pattern.
pub fn detect_loadlibrary_injection(events: &[EtwTiEvent]) -> Vec<InjectionDetection> {
    // Track pending alloc events keyed by (caller_pid, target_pid, base_address).
    let mut pending_allocs: HashMap<(u32, u32, String), &EtwTiEvent> = HashMap::new();
    // Track confirmed write events keyed by (caller_pid, target_pid).
    let mut confirmed_writes: HashMap<(u32, u32), &EtwTiEvent> = HashMap::new();
    let mut detections = Vec::new();

    for event in events {
        let caller = match event.calling_process_id {
            Some(pid) => pid,
            None => continue,
        };
        let target = match event.target_process_id {
            Some(pid) if pid != caller => pid,
            _ => continue,
        };

        match EtwTiEventId::from_id(event.event_id) {
            Some(EtwTiEventId::AllocVmRemote) => {
                if let Some(addr) = &event.base_address {
                    pending_allocs.insert((caller, target, addr.clone()), event);
                }
            }
            Some(EtwTiEventId::WriteVmRemote) => {
                if let Some(addr) = &event.base_address {
                    let key = (caller, target, addr.clone());
                    if pending_allocs.contains_key(&key) {
                        confirmed_writes.insert((caller, target), event);
                    }
                }
            }
            _ => {
                // ImageLoad events from Kernel-Process provider have no
                // CallingProcessId. They are matched separately
                // below.
            }
        }
    }

    // Build detections from confirmed writes.
    for ((caller, target), write_ev) in &confirmed_writes {
        let alloc_ev = pending_allocs
            .iter()
            .find(|((c, t, _), _)| c == caller && t == target)
            .map(|(_, ev)| *ev);

        // Look for a matching ImageLoad in the target process.
        let image_ev = events.iter().find(|ev| {
            ev.event_id == 5
                && ev.provider.contains("Kernel-Process")
                && ev.calling_process_id == Some(*target)
                && ev.image_name.is_some()
        });

        detections.push(InjectionDetection {
            injector_pid: *caller,
            injector_name: write_ev.calling_process_name.clone(),
            target_pid: *target,
            target_name: write_ev.target_process_name.clone(),
            alloc_address: alloc_ev.and_then(|e| e.base_address.clone()),
            image_name: image_ev.and_then(|e| e.image_name.clone()),
            first_seen: alloc_ev
                .map(|e| e.timestamp.clone())
                .unwrap_or_else(|| write_ev.timestamp.clone()),
        });
    }

    detections
}

/// Parse a multi-line JSONL string and return all successfully parsed events.
pub fn parse_jsonl(jsonl: &str) -> Vec<EtwTiEvent> {
    jsonl
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(EtwTiEvent::from_jsonl)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_JSONL: &str = r#"
{"EventId":1,"Provider":"Microsoft-Windows-Threat-Intelligence","Keyword":"ALLOCVM_REMOTE","Timestamp":"2026-04-13T14:32:11.0012345Z","CallingProcessId":4812,"CallingProcessName":"textquest.exe","CallingThreadId":9120,"TargetProcessId":7744,"TargetProcessName":"eqgame.exe","BaseAddress":"0x000001F3A2C00000","RegionSize":516,"AllocationType":"0x3000","Protect":"0x4","ProtectName":"PAGE_READWRITE"}
{"EventId":2,"Provider":"Microsoft-Windows-Threat-Intelligence","Keyword":"WRITEVM_REMOTE","Timestamp":"2026-04-13T14:32:11.0013210Z","CallingProcessId":4812,"CallingProcessName":"textquest.exe","CallingThreadId":9120,"TargetProcessId":7744,"TargetProcessName":"eqgame.exe","BaseAddress":"0x000001F3A2C00000","ByteCount":516}
{"EventId":5,"Provider":"Microsoft-Windows-Kernel-Process","Keyword":"IMAGELOAD","Timestamp":"2026-04-13T14:32:11.0891002Z","CallingProcessId":7744,"ImageName":"C:\\ProgramData\\textquest\\textquest_dll.dll","ImageBase":"0x000001F3B4A00000","ImageSize":2097152}
"#;

    #[test]
    fn parse_sample_jsonl() {
        let events = parse_jsonl(SAMPLE_JSONL);
        assert_eq!(events.len(), 3, "expected 3 parsed events");

        let alloc = &events[0];
        assert_eq!(alloc.event_id, 1);
        assert_eq!(alloc.calling_process_id, Some(4812));
        assert_eq!(alloc.target_process_id, Some(7744));
        assert_eq!(alloc.base_address.as_deref(), Some("0x000001F3A2C00000"));

        let write = &events[1];
        assert_eq!(write.event_id, 2);
        assert_eq!(write.byte_count, Some(516));

        let image = &events[2];
        assert_eq!(image.event_id, 5);
        assert!(
            image
                .image_name
                .as_deref()
                .unwrap()
                .contains("textquest_dll")
        );
    }

    #[test]
    fn detect_loadlibrary_injection_from_sample() {
        let events = parse_jsonl(SAMPLE_JSONL);
        let detections = detect_loadlibrary_injection(&events);
        assert_eq!(
            detections.len(),
            1,
            "expected exactly one injection detection"
        );

        let d = &detections[0];
        assert_eq!(d.injector_pid, 4812);
        assert_eq!(d.target_pid, 7744);
        assert_eq!(d.injector_name.as_deref(), Some("textquest.exe"));
        assert_eq!(d.target_name.as_deref(), Some("eqgame.exe"));
        assert!(d.alloc_address.is_some());
    }

    #[test]
    fn no_detection_without_write_event() {
        // Only an ALLOCVM_REMOTE event — not enough to confirm injection.
        let jsonl = r#"{"EventId":1,"Provider":"Microsoft-Windows-Threat-Intelligence","Keyword":"ALLOCVM_REMOTE","Timestamp":"2026-04-13T00:00:00Z","CallingProcessId":100,"CallingProcessName":"foo.exe","TargetProcessId":200,"TargetProcessName":"bar.exe","BaseAddress":"0x1000"}"#;
        let events = parse_jsonl(jsonl);
        let detections = detect_loadlibrary_injection(&events);
        assert!(
            detections.is_empty(),
            "alloc-only should not trigger detection"
        );
    }

    #[test]
    fn no_detection_same_pid() {
        // CallingProcessId == TargetProcessId — self-alloc, not injection.
        let jsonl = r#"{"EventId":1,"Provider":"Microsoft-Windows-Threat-Intelligence","Keyword":"ALLOCVM_REMOTE","Timestamp":"2026-04-13T00:00:00Z","CallingProcessId":100,"CallingProcessName":"self.exe","TargetProcessId":100,"TargetProcessName":"self.exe","BaseAddress":"0x1000"}
{"EventId":2,"Provider":"Microsoft-Windows-Threat-Intelligence","Keyword":"WRITEVM_REMOTE","Timestamp":"2026-04-13T00:00:01Z","CallingProcessId":100,"CallingProcessName":"self.exe","TargetProcessId":100,"TargetProcessName":"self.exe","BaseAddress":"0x1000","ByteCount":64}"#;
        let events = parse_jsonl(jsonl);
        let detections = detect_loadlibrary_injection(&events);
        assert!(
            detections.is_empty(),
            "same-pid alloc/write should not trigger detection"
        );
    }

    #[test]
    fn etw_ti_event_id_round_trip() {
        assert_eq!(EtwTiEventId::from_id(1), Some(EtwTiEventId::AllocVmRemote));
        assert_eq!(EtwTiEventId::from_id(2), Some(EtwTiEventId::WriteVmRemote));
        assert_eq!(EtwTiEventId::from_id(5), Some(EtwTiEventId::ReadVmRemote));
        assert_eq!(EtwTiEventId::from_id(99), None);
    }

    // ─── EtwTiEventId additional coverage ────────────────────────────────

    #[test]
    fn etw_ti_event_id_all_variants() {
        assert_eq!(
            EtwTiEventId::from_id(10),
            Some(EtwTiEventId::SetThreadContext)
        );
        assert_eq!(EtwTiEventId::from_id(12), Some(EtwTiEventId::MapViewRemote));
        assert_eq!(
            EtwTiEventId::from_id(14),
            Some(EtwTiEventId::QueueUserApcRemote)
        );
    }

    #[test]
    fn etw_ti_event_id_zero_returns_none() {
        assert_eq!(EtwTiEventId::from_id(0), None);
    }

    #[test]
    fn etw_ti_event_id_boundary_values() {
        // Values adjacent to valid IDs
        assert_eq!(EtwTiEventId::from_id(3), None);
        assert_eq!(EtwTiEventId::from_id(4), None);
        assert_eq!(EtwTiEventId::from_id(6), None);
        assert_eq!(EtwTiEventId::from_id(11), None);
        assert_eq!(EtwTiEventId::from_id(13), None);
        assert_eq!(EtwTiEventId::from_id(15), None);
    }

    // ─── from_jsonl edge cases ───────────────────────────────────────────

    #[test]
    fn from_jsonl_empty_string() {
        assert!(EtwTiEvent::from_jsonl("").is_none());
    }

    #[test]
    fn from_jsonl_invalid_json() {
        assert!(EtwTiEvent::from_jsonl("not json").is_none());
    }

    #[test]
    fn from_jsonl_missing_event_id() {
        let line = r#"{"Provider":"foo","Keyword":"bar"}"#;
        assert!(EtwTiEvent::from_jsonl(line).is_none());
    }

    #[test]
    fn from_jsonl_minimal_event() {
        let line = r#"{"EventId":1}"#;
        let event = EtwTiEvent::from_jsonl(line).expect("should parse minimal event");
        assert_eq!(event.event_id, 1);
        assert_eq!(event.provider, "");
        assert_eq!(event.keyword, "");
        assert!(event.calling_process_id.is_none());
        assert!(event.target_process_id.is_none());
    }

    #[test]
    fn from_jsonl_extra_fields_captured() {
        let line = r#"{"EventId":1,"CustomField":"hello","NumericExtra":42}"#;
        let event = EtwTiEvent::from_jsonl(line).expect("should parse");
        assert_eq!(event.extra.get("CustomField").unwrap(), "hello");
        assert_eq!(event.extra.get("NumericExtra").unwrap(), 42);
    }

    #[test]
    fn from_jsonl_whitespace_trimming() {
        let line = r#"  {"EventId":2}  "#;
        let event = EtwTiEvent::from_jsonl(line).expect("should parse trimmed");
        assert_eq!(event.event_id, 2);
    }

    // ─── parse_jsonl edge cases ──────────────────────────────────────────

    #[test]
    fn parse_jsonl_empty_input() {
        let events = parse_jsonl("");
        assert!(events.is_empty());
    }

    #[test]
    fn parse_jsonl_all_blank_lines() {
        let events = parse_jsonl("\n\n\n");
        assert!(events.is_empty());
    }

    #[test]
    fn parse_jsonl_skips_invalid_lines() {
        let input = r#"{"EventId":1}
not valid json
{"EventId":2}
also invalid
{"EventId":5}"#;
        let events = parse_jsonl(input);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_id, 1);
        assert_eq!(events[1].event_id, 2);
        assert_eq!(events[2].event_id, 5);
    }

    // ─── detect_loadlibrary_injection edge cases ─────────────────────────

    #[test]
    fn detect_injection_alloc_write_no_imageload() {
        // Alloc + Write without ImageLoad should still detect
        let jsonl = r#"{"EventId":1,"Provider":"Microsoft-Windows-Threat-Intelligence","Keyword":"ALLOCVM_REMOTE","Timestamp":"2026-01-01T00:00:00Z","CallingProcessId":100,"TargetProcessId":200,"BaseAddress":"0xABC0"}
{"EventId":2,"Provider":"Microsoft-Windows-Threat-Intelligence","Keyword":"WRITEVM_REMOTE","Timestamp":"2026-01-01T00:00:01Z","CallingProcessId":100,"TargetProcessId":200,"BaseAddress":"0xABC0","ByteCount":256}"#;
        let events = parse_jsonl(jsonl);
        let detections = detect_loadlibrary_injection(&events);
        assert_eq!(detections.len(), 1);
        assert_eq!(detections[0].injector_pid, 100);
        assert_eq!(detections[0].target_pid, 200);
        assert!(detections[0].image_name.is_none());
    }

    #[test]
    fn detect_injection_mismatched_addresses() {
        // Alloc at address A, Write at address B — should NOT detect
        let jsonl = r#"{"EventId":1,"Provider":"Microsoft-Windows-Threat-Intelligence","Keyword":"ALLOCVM_REMOTE","Timestamp":"2026-01-01T00:00:00Z","CallingProcessId":100,"TargetProcessId":200,"BaseAddress":"0x1000"}
{"EventId":2,"Provider":"Microsoft-Windows-Threat-Intelligence","Keyword":"WRITEVM_REMOTE","Timestamp":"2026-01-01T00:00:01Z","CallingProcessId":100,"TargetProcessId":200,"BaseAddress":"0x2000","ByteCount":64}"#;
        let events = parse_jsonl(jsonl);
        let detections = detect_loadlibrary_injection(&events);
        assert!(
            detections.is_empty(),
            "mismatched base addresses should not trigger detection"
        );
    }

    #[test]
    fn detect_injection_write_without_alloc() {
        // Write event without preceding Alloc — should NOT detect
        let jsonl = r#"{"EventId":2,"Provider":"Microsoft-Windows-Threat-Intelligence","Keyword":"WRITEVM_REMOTE","Timestamp":"2026-01-01T00:00:00Z","CallingProcessId":100,"TargetProcessId":200,"BaseAddress":"0x1000","ByteCount":64}"#;
        let events = parse_jsonl(jsonl);
        let detections = detect_loadlibrary_injection(&events);
        assert!(detections.is_empty(), "write-only should not trigger");
    }

    #[test]
    fn detect_injection_empty_events() {
        let detections = detect_loadlibrary_injection(&[]);
        assert!(detections.is_empty());
    }

    #[test]
    fn detect_injection_events_without_pids() {
        let jsonl = r#"{"EventId":1,"Provider":"Microsoft-Windows-Threat-Intelligence","Keyword":"ALLOCVM_REMOTE","Timestamp":"2026-01-01T00:00:00Z","BaseAddress":"0x1000"}"#;
        let events = parse_jsonl(jsonl);
        let detections = detect_loadlibrary_injection(&events);
        assert!(
            detections.is_empty(),
            "events without PIDs should be skipped"
        );
    }
}
