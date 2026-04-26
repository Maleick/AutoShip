use serde::Serialize;
use std::collections::BTreeMap;

pub const PIPE_EVENT_MAGIC: u32 = 0x5449_5445;
pub const PIPE_MAX_MSG_BYTES: usize = 4096;

const PIPE_HEADER_LEGACY_LEN: usize = 26;
const PIPE_HEADER_CURRENT_LEN: usize = 36;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOptions {
    pub process_name: String,
    pub keyword_mask: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TiEvent {
    pub ts: u64,
    pub event_id: u16,
    pub task: u16,
    pub keyword: u64,
    pub pid: u32,
    pub tid: u32,
    pub name: String,
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConsumerError {
    #[error("missing required argument {0}")]
    MissingArgument(&'static str),
    #[error("missing value for {0}")]
    MissingValue(&'static str),
    #[error("unknown argument {0}")]
    UnknownArgument(String),
    #[error("unknown ETW-TI keyword {0}")]
    UnknownKeyword(String),
    #[error("pipe message is too short: expected at least {expected} bytes, got {actual}")]
    TruncatedHeader { expected: usize, actual: usize },
    #[error("bad pipe magic 0x{0:08X}")]
    BadMagic(u32),
    #[error("pipe payload is truncated: expected {expected} bytes, got {actual}")]
    TruncatedPayload { expected: usize, actual: usize },
    #[error("pipe text is not valid UTF-8: {0}")]
    Utf8(String),
    #[error("json serialization failed: {0}")]
    Json(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeywordSpec {
    pub name: &'static str,
    pub task: u16,
    pub bit: u64,
}

pub const KEYWORDS: &[KeywordSpec] = &[
    KeywordSpec {
        name: "ALLOCVM_LOCAL",
        task: 1,
        bit: 0x1,
    },
    KeywordSpec {
        name: "ALLOCVM_LOCAL_KERNEL_CALLER",
        task: 1,
        bit: 0x2,
    },
    KeywordSpec {
        name: "ALLOCVM_REMOTE",
        task: 1,
        bit: 0x4,
    },
    KeywordSpec {
        name: "ALLOCVM_REMOTE_KERNEL_CALLER",
        task: 1,
        bit: 0x8,
    },
    KeywordSpec {
        name: "PROTECTVM_LOCAL",
        task: 2,
        bit: 0x10,
    },
    KeywordSpec {
        name: "PROTECTVM_LOCAL_KERNEL_CALLER",
        task: 2,
        bit: 0x20,
    },
    KeywordSpec {
        name: "PROTECTVM_REMOTE",
        task: 2,
        bit: 0x40,
    },
    KeywordSpec {
        name: "PROTECTVM_REMOTE_KERNEL_CALLER",
        task: 2,
        bit: 0x80,
    },
    KeywordSpec {
        name: "MAPVIEW_LOCAL",
        task: 3,
        bit: 0x100,
    },
    KeywordSpec {
        name: "MAPVIEW_LOCAL_KERNEL_CALLER",
        task: 3,
        bit: 0x200,
    },
    KeywordSpec {
        name: "MAPVIEW_REMOTE",
        task: 3,
        bit: 0x400,
    },
    KeywordSpec {
        name: "MAPVIEW_REMOTE_KERNEL_CALLER",
        task: 3,
        bit: 0x800,
    },
    KeywordSpec {
        name: "QUEUEUSERAPC_REMOTE",
        task: 4,
        bit: 0x1000,
    },
    KeywordSpec {
        name: "QUEUEUSERAPC_REMOTE_KERNEL_CALLER",
        task: 4,
        bit: 0x2000,
    },
    KeywordSpec {
        name: "SETTHREADCONTEXT_REMOTE",
        task: 5,
        bit: 0x4000,
    },
    KeywordSpec {
        name: "SETTHREADCONTEXT_REMOTE_KERNEL_CALLER",
        task: 5,
        bit: 0x8000,
    },
    KeywordSpec {
        name: "READVM_LOCAL",
        task: 6,
        bit: 0x1_0000,
    },
    KeywordSpec {
        name: "READVM_REMOTE",
        task: 6,
        bit: 0x2_0000,
    },
    KeywordSpec {
        name: "WRITEVM_LOCAL",
        task: 7,
        bit: 0x4_0000,
    },
    KeywordSpec {
        name: "WRITEVM_REMOTE",
        task: 7,
        bit: 0x8_0000,
    },
    KeywordSpec {
        name: "SUSPEND_THREAD",
        task: 8,
        bit: 0x10_0000,
    },
    KeywordSpec {
        name: "RESUME_THREAD",
        task: 8,
        bit: 0x20_0000,
    },
    KeywordSpec {
        name: "SUSPEND_PROCESS",
        task: 9,
        bit: 0x40_0000,
    },
    KeywordSpec {
        name: "RESUME_PROCESS",
        task: 9,
        bit: 0x80_0000,
    },
    KeywordSpec {
        name: "FREEZE_PROCESS",
        task: 9,
        bit: 0x100_0000,
    },
    KeywordSpec {
        name: "THAW_PROCESS",
        task: 9,
        bit: 0x200_0000,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessFilter {
    expected: Option<String>,
}

impl ProcessFilter {
    pub fn for_name(name: impl AsRef<str>) -> Self {
        let normalized = normalize_process_name(name.as_ref());
        let expected = (!normalized.is_empty()).then_some(normalized);
        Self { expected }
    }

    pub fn disabled() -> Self {
        Self { expected: None }
    }

    pub fn matches_process_name(&self, process_name: &str) -> bool {
        match &self.expected {
            Some(expected) => normalize_process_name(process_name) == *expected,
            None => true,
        }
    }
}

pub fn usage() -> &'static str {
    "Usage: etw-consumer.exe --process eqgame.exe --keywords ALLOCVM_REMOTE,PROTECTVM_REMOTE,WRITEVM_REMOTE,READVM_REMOTE,SETTHREADCONTEXT_REMOTE"
}

pub fn parse_cli_args<I, S>(args: I) -> Result<CliOptions, ConsumerError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut iter = args.into_iter().map(Into::into);
    let mut process_name = None;
    let mut keywords = None;

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--process" | "--process-name" => {
                process_name = Some(
                    iter.next()
                        .ok_or(ConsumerError::MissingValue("--process"))?,
                );
            }
            "--keywords" => {
                keywords = Some(
                    iter.next()
                        .ok_or(ConsumerError::MissingValue("--keywords"))?,
                );
            }
            _ => return Err(ConsumerError::UnknownArgument(arg)),
        }
    }

    let process_name = process_name.ok_or(ConsumerError::MissingArgument("--process"))?;
    let keyword_mask = parse_keyword_mask(
        keywords
            .as_deref()
            .ok_or(ConsumerError::MissingArgument("--keywords"))?,
    )?;

    Ok(CliOptions {
        process_name,
        keyword_mask,
    })
}

pub fn parse_keyword_mask(input: &str) -> Result<u64, ConsumerError> {
    let trimmed = input.trim();
    if trimmed.eq_ignore_ascii_case("all") || trimmed == "*" {
        return Ok(KEYWORDS.iter().fold(0, |mask, spec| mask | spec.bit));
    }

    let mut mask = 0;
    for raw in trimmed.split(',') {
        let name = raw.trim();
        if name.is_empty() {
            continue;
        }
        if let Some(hex) = name.strip_prefix("0x").or_else(|| name.strip_prefix("0X")) {
            let bit = u64::from_str_radix(hex, 16)
                .map_err(|_| ConsumerError::UnknownKeyword(name.to_string()))?;
            mask |= bit;
            continue;
        }
        let spec = KEYWORDS
            .iter()
            .find(|spec| spec.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| ConsumerError::UnknownKeyword(name.to_string()))?;
        mask |= spec.bit;
    }

    Ok(mask)
}

pub fn keyword_name(task: u16, keyword: u64, fallback: &str) -> String {
    if !fallback.is_empty() {
        return fallback.to_string();
    }

    KEYWORDS
        .iter()
        .rev()
        .find(|spec| spec.task == task && keyword & spec.bit != 0)
        .map(|spec| spec.name.to_string())
        .unwrap_or_else(|| format!("UNKNOWN_TASK_{task}_0x{keyword:016X}"))
}

pub fn normalize_process_name(input: &str) -> String {
    input
        .trim()
        .trim_matches('"')
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

pub fn decode_pipe_message(bytes: &[u8]) -> Result<TiEvent, ConsumerError> {
    if bytes.len() < PIPE_HEADER_LEGACY_LEN {
        return Err(ConsumerError::TruncatedHeader {
            expected: PIPE_HEADER_LEGACY_LEN,
            actual: bytes.len(),
        });
    }

    let magic = read_u32(bytes, 0);
    if magic != PIPE_EVENT_MAGIC {
        return Err(ConsumerError::BadMagic(magic));
    }

    if bytes.len() >= PIPE_HEADER_CURRENT_LEN {
        let current_name_len = read_u16(bytes, 32) as usize;
        let current_fields_len = read_u16(bytes, 34) as usize;
        let current_expected = PIPE_HEADER_CURRENT_LEN + current_name_len + current_fields_len;
        if current_expected <= bytes.len() && current_expected <= PIPE_MAX_MSG_BYTES {
            return decode_current_pipe_message(bytes, current_name_len, current_fields_len);
        }
    }

    decode_legacy_pipe_message(bytes)
}

pub fn parse_fields_blob(bytes: &[u8]) -> Result<BTreeMap<String, String>, ConsumerError> {
    let mut fields = BTreeMap::new();
    for entry in bytes.split(|byte| *byte == 0) {
        if entry.is_empty() {
            continue;
        }
        let pair =
            std::str::from_utf8(entry).map_err(|err| ConsumerError::Utf8(err.to_string()))?;
        match pair.split_once('=') {
            Some((key, value)) => {
                fields.insert(key.to_string(), value.to_string());
            }
            None => {
                fields.insert(pair.to_string(), String::new());
            }
        }
    }
    Ok(fields)
}

pub fn event_to_jsonl(event: &TiEvent, process: &str) -> Result<String, ConsumerError> {
    #[derive(Serialize)]
    struct JsonEvent<'a> {
        ts: u64,
        keyword: String,
        pid: u32,
        process: &'a str,
        fields: &'a BTreeMap<String, String>,
    }

    let json = JsonEvent {
        ts: event.ts,
        keyword: keyword_name(event.task, event.keyword, &event.name),
        pid: event.pid,
        process,
        fields: &event.fields,
    };

    serde_json::to_string(&json).map_err(|err| ConsumerError::Json(err.to_string()))
}

pub fn run(options: CliOptions) -> anyhow::Result<()> {
    platform::run(options)
}

fn decode_current_pipe_message(
    bytes: &[u8],
    name_len: usize,
    fields_len: usize,
) -> Result<TiEvent, ConsumerError> {
    let expected = PIPE_HEADER_CURRENT_LEN + name_len + fields_len;
    if bytes.len() < expected {
        return Err(ConsumerError::TruncatedPayload {
            expected,
            actual: bytes.len(),
        });
    }

    let name_start = PIPE_HEADER_CURRENT_LEN;
    let fields_start = name_start + name_len;
    let name = std::str::from_utf8(&bytes[name_start..fields_start])
        .map_err(|err| ConsumerError::Utf8(err.to_string()))?
        .to_string();

    Ok(TiEvent {
        ts: read_u64(bytes, 24),
        event_id: read_u16(bytes, 4),
        task: read_u16(bytes, 6),
        keyword: read_u64(bytes, 8),
        pid: read_u32(bytes, 16),
        tid: read_u32(bytes, 20),
        name,
        fields: parse_fields_blob(&bytes[fields_start..fields_start + fields_len])?,
    })
}

fn decode_legacy_pipe_message(bytes: &[u8]) -> Result<TiEvent, ConsumerError> {
    let name_len = read_u16(bytes, 22) as usize;
    let fields_len = read_u16(bytes, 24) as usize;
    let expected = PIPE_HEADER_LEGACY_LEN + name_len + fields_len;
    if bytes.len() < expected {
        return Err(ConsumerError::TruncatedPayload {
            expected,
            actual: bytes.len(),
        });
    }

    let name_start = PIPE_HEADER_LEGACY_LEN;
    let fields_start = name_start + name_len;
    let name = std::str::from_utf8(&bytes[name_start..fields_start])
        .map_err(|err| ConsumerError::Utf8(err.to_string()))?
        .to_string();

    Ok(TiEvent {
        ts: read_u64(bytes, 14),
        event_id: read_u16(bytes, 4),
        task: 0,
        keyword: 0,
        pid: read_u32(bytes, 6),
        tid: read_u32(bytes, 10),
        name,
        fields: parse_fields_blob(&bytes[fields_start..fields_start + fields_len])?,
    })
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("valid u16 range"),
    )
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("valid u32 range"),
    )
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("valid u64 range"),
    )
}

#[cfg(not(windows))]
mod platform {
    use super::CliOptions;

    pub fn run(_options: CliOptions) -> anyhow::Result<()> {
        anyhow::bail!(
            "etw-consumer can only run on Windows because ETW-TI, TDH, DeviceIoControl, and the named pipe source are Windows APIs"
        )
    }
}

#[cfg(windows)]
mod platform {
    use super::{
        CliOptions, PIPE_MAX_MSG_BYTES, ProcessFilter, TiEvent, decode_pipe_message,
        event_to_jsonl, keyword_name,
    };
    use std::collections::{BTreeMap, HashMap};
    use std::ffi::OsStr;
    use std::io::Write;
    use std::mem::{size_of, zeroed};
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::{null, null_mut};
    use std::sync::{Mutex, OnceLock, mpsc};
    use std::thread;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_ACCESS_DENIED, ERROR_SUCCESS, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE, OPEN_EXISTING,
        ReadFile,
    };
    use windows_sys::Win32::System::Diagnostics::Etw::{
        CONTROLTRACE_HANDLE, CloseTrace, ControlTraceW, ENABLE_TRACE_PARAMETERS,
        ENABLE_TRACE_PARAMETERS_VERSION_2, EVENT_CONTROL_CODE_ENABLE_PROVIDER, EVENT_PROPERTY_INFO,
        EVENT_RECORD, EVENT_TRACE_CONTROL_STOP, EVENT_TRACE_LOGFILEW, EVENT_TRACE_PROPERTIES,
        EVENT_TRACE_REAL_TIME_MODE, EnableTraceEx2, OpenTraceW, PROCESS_TRACE_MODE_EVENT_RECORD,
        PROCESS_TRACE_MODE_REAL_TIME, PROCESSTRACE_HANDLE, PROPERTY_DATA_DESCRIPTOR, ProcessTrace,
        PropertyStruct, StartTraceW, TDH_INTYPE_ANSISTRING, TDH_INTYPE_BOOLEAN,
        TDH_INTYPE_FILETIME, TDH_INTYPE_GUID, TDH_INTYPE_HEXINT32, TDH_INTYPE_HEXINT64,
        TDH_INTYPE_INT8, TDH_INTYPE_INT16, TDH_INTYPE_INT32, TDH_INTYPE_INT64, TDH_INTYPE_POINTER,
        TDH_INTYPE_UINT8, TDH_INTYPE_UINT16, TDH_INTYPE_UINT32, TDH_INTYPE_UINT64,
        TDH_INTYPE_UNICODESTRING, TRACE_EVENT_INFO, TRACE_LEVEL_VERBOSE, TdhGetEventInformation,
        TdhGetProperty, TdhGetPropertySize, WNODE_FLAG_TRACED_GUID,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use windows_sys::core::GUID;

    const SESSION_NAME: &str = "TextQuestEtwTiConsumer";
    const ETWTI_DEVICE_PATH: &str = r"\\.\EtwTiDriver";
    const ETWTI_PIPE_PATH: &str = r"\\.\pipe\EtwTiForwarder";
    const ETW_TI_PROVIDER: GUID = GUID::from_u128(0xf4e1897c_bb5d_5668_f1d8_040f4d8dd344);
    const INVALID_PROCESSTRACE_VALUE: u64 = u64::MAX;

    const FILE_DEVICE_UNKNOWN: u32 = 0x22;
    const METHOD_BUFFERED: u32 = 0;
    const FILE_ANY_ACCESS: u32 = 0;
    const IOCTL_ETWTI_START: u32 =
        ctl_code(FILE_DEVICE_UNKNOWN, 0x800, METHOD_BUFFERED, FILE_ANY_ACCESS);
    const IOCTL_ETWTI_STOP: u32 =
        ctl_code(FILE_DEVICE_UNKNOWN, 0x801, METHOD_BUFFERED, FILE_ANY_ACCESS);

    static EVENT_SENDER: OnceLock<Mutex<Option<mpsc::Sender<TiEvent>>>> = OnceLock::new();

    pub fn run(options: CliOptions) -> anyhow::Result<()> {
        let (sender, receiver) = mpsc::channel();
        let _pipe_thread = spawn_pipe_reader(sender.clone());
        let driver = DriverControl::open();
        if let Ok(driver) = &driver {
            driver.start(options.keyword_mask)?;
        } else if let Err(err) = &driver {
            eprintln!("warning: driver control unavailable: {err}");
        }

        let _session = EtwSession::start(options.keyword_mask, sender)?;
        let filter = ProcessFilter::for_name(options.process_name);
        let mut resolver = ProcessResolver::default();
        let stdout = std::io::stdout();
        let mut stdout = stdout.lock();

        while let Ok(event) = receiver.recv() {
            let process = resolver.resolve(event.pid);
            if filter.matches_process_name(&process) {
                writeln!(stdout, "{}", event_to_jsonl(&event, &process)?)?;
            }
        }

        if let Ok(driver) = driver {
            driver.stop()?;
        }
        Ok(())
    }

    const fn ctl_code(device_type: u32, function: u32, method: u32, access: u32) -> u32 {
        (device_type << 16) | (access << 14) | (function << 2) | method
    }

    #[repr(C)]
    struct EtwTiStartInput {
        keyword_mask: u64,
    }

    struct DriverControl {
        handle: HANDLE,
    }

    impl DriverControl {
        fn open() -> anyhow::Result<Self> {
            let path = wide(ETWTI_DEVICE_PATH);
            let handle = unsafe {
                CreateFileW(
                    path.as_ptr(),
                    FILE_GENERIC_READ | FILE_GENERIC_WRITE,
                    0,
                    null(),
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    null_mut(),
                )
            };
            if handle == INVALID_HANDLE_VALUE {
                anyhow::bail!("CreateFileW({ETWTI_DEVICE_PATH}) failed: {}", unsafe {
                    GetLastError()
                });
            }
            Ok(Self { handle })
        }

        fn start(&self, keyword_mask: u64) -> anyhow::Result<()> {
            let input = EtwTiStartInput { keyword_mask };
            let mut bytes_returned = 0;
            let ok = unsafe {
                DeviceIoControl(
                    self.handle,
                    IOCTL_ETWTI_START,
                    &input as *const _ as *const _,
                    size_of::<EtwTiStartInput>() as u32,
                    null_mut(),
                    0,
                    &mut bytes_returned,
                    null_mut(),
                )
            };
            if ok == 0 {
                anyhow::bail!("IOCTL_ETWTI_START failed: {}", unsafe { GetLastError() });
            }
            Ok(())
        }

        fn stop(&self) -> anyhow::Result<()> {
            let mut bytes_returned = 0;
            let ok = unsafe {
                DeviceIoControl(
                    self.handle,
                    IOCTL_ETWTI_STOP,
                    null(),
                    0,
                    null_mut(),
                    0,
                    &mut bytes_returned,
                    null_mut(),
                )
            };
            if ok == 0 {
                anyhow::bail!("IOCTL_ETWTI_STOP failed: {}", unsafe { GetLastError() });
            }
            Ok(())
        }
    }

    impl Drop for DriverControl {
        fn drop(&mut self) {
            if self.handle != INVALID_HANDLE_VALUE {
                let _ = self.stop();
                unsafe {
                    CloseHandle(self.handle);
                }
            }
        }
    }

    struct EtwSession {
        session: CONTROLTRACE_HANDLE,
        trace: PROCESSTRACE_HANDLE,
        thread: Option<thread::JoinHandle<()>>,
        session_name: Vec<u16>,
    }

    impl EtwSession {
        fn start(keyword_mask: u64, sender: mpsc::Sender<TiEvent>) -> anyhow::Result<Self> {
            let session_name = wide(SESSION_NAME);
            stop_orphan_session(&session_name);

            let sender_slot = EVENT_SENDER.get_or_init(|| Mutex::new(None));
            *sender_slot.lock().expect("event sender lock poisoned") = Some(sender);

            let mut props = trace_properties(&session_name);
            let props_ptr = props.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
            let mut session = CONTROLTRACE_HANDLE::default();
            let status = unsafe { StartTraceW(&mut session, session_name.as_ptr(), props_ptr) };
            if status != ERROR_SUCCESS {
                clear_event_sender();
                anyhow::bail!("StartTraceW failed: {status}");
            }

            let enable = ENABLE_TRACE_PARAMETERS {
                Version: ENABLE_TRACE_PARAMETERS_VERSION_2,
                ..unsafe { zeroed() }
            };
            let status = unsafe {
                EnableTraceEx2(
                    session,
                    &ETW_TI_PROVIDER,
                    EVENT_CONTROL_CODE_ENABLE_PROVIDER,
                    TRACE_LEVEL_VERBOSE as u8,
                    keyword_mask,
                    0,
                    0,
                    &enable,
                )
            };
            if status != ERROR_SUCCESS {
                stop_session(session, &session_name);
                clear_event_sender();
                if status == ERROR_ACCESS_DENIED {
                    anyhow::bail!(
                        "EnableTraceEx2 access denied. Patch EPROCESS->Protection to 0x31 for this process before starting capture."
                    );
                }
                anyhow::bail!("EnableTraceEx2 failed: {status}");
            }

            let mut logfile = EVENT_TRACE_LOGFILEW::default();
            logfile.LoggerName = session_name.as_ptr() as *mut _;
            logfile.Anonymous1.ProcessTraceMode =
                PROCESS_TRACE_MODE_REAL_TIME | PROCESS_TRACE_MODE_EVENT_RECORD;
            logfile.Anonymous2.EventRecordCallback = Some(event_record_callback);

            let trace = unsafe { OpenTraceW(&mut logfile) };
            if trace.Value == INVALID_PROCESSTRACE_VALUE {
                stop_session(session, &session_name);
                clear_event_sender();
                anyhow::bail!("OpenTraceW failed: {}", unsafe { GetLastError() });
            }

            let process_trace_handle = trace;
            let thread = thread::spawn(move || unsafe {
                ProcessTrace(&process_trace_handle, 1, null(), null());
            });

            Ok(Self {
                session,
                trace,
                thread: Some(thread),
                session_name,
            })
        }
    }

    impl Drop for EtwSession {
        fn drop(&mut self) {
            if self.trace.Value != INVALID_PROCESSTRACE_VALUE {
                unsafe {
                    CloseTrace(self.trace);
                }
            }
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
            stop_session(self.session, &self.session_name);
            clear_event_sender();
        }
    }

    fn stop_orphan_session(session_name: &[u16]) {
        stop_session(CONTROLTRACE_HANDLE::default(), session_name);
    }

    fn stop_session(session: CONTROLTRACE_HANDLE, session_name: &[u16]) {
        let mut props = trace_properties(session_name);
        let props_ptr = props.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
        unsafe {
            ControlTraceW(
                session,
                session_name.as_ptr(),
                props_ptr,
                EVENT_TRACE_CONTROL_STOP,
            );
        }
    }

    fn trace_properties(session_name: &[u16]) -> Vec<u8> {
        let props_len = size_of::<EVENT_TRACE_PROPERTIES>();
        let name_len = session_name.len() * size_of::<u16>();
        let mut bytes = vec![0; props_len + name_len];
        unsafe {
            let props = bytes.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES;
            (*props).Wnode.BufferSize = bytes.len() as u32;
            (*props).Wnode.Flags = WNODE_FLAG_TRACED_GUID;
            (*props).Wnode.ClientContext = 1;
            (*props).LogFileMode = EVENT_TRACE_REAL_TIME_MODE;
            (*props).LoggerNameOffset = props_len as u32;
            std::ptr::copy_nonoverlapping(
                session_name.as_ptr() as *const u8,
                bytes.as_mut_ptr().add(props_len),
                name_len,
            );
        }
        bytes
    }

    fn spawn_pipe_reader(sender: mpsc::Sender<TiEvent>) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            loop {
                match open_pipe() {
                    Ok(pipe) => {
                        read_pipe_loop(pipe, &sender);
                        unsafe {
                            CloseHandle(pipe);
                        }
                    }
                    Err(err) => {
                        eprintln!("warning: pipe source unavailable: {err}");
                        thread::sleep(Duration::from_millis(500));
                    }
                }
            }
        })
    }

    fn open_pipe() -> anyhow::Result<HANDLE> {
        let path = wide(ETWTI_PIPE_PATH);
        let pipe = unsafe {
            CreateFileW(
                path.as_ptr(),
                FILE_GENERIC_READ,
                0,
                null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            )
        };
        if pipe == INVALID_HANDLE_VALUE {
            anyhow::bail!("CreateFileW({ETWTI_PIPE_PATH}) failed: {}", unsafe {
                GetLastError()
            });
        }
        Ok(pipe)
    }

    fn read_pipe_loop(pipe: HANDLE, sender: &mpsc::Sender<TiEvent>) {
        let mut buf = vec![0u8; PIPE_MAX_MSG_BYTES];
        loop {
            let mut bytes_read = 0;
            let ok = unsafe {
                ReadFile(
                    pipe,
                    buf.as_mut_ptr(),
                    buf.len() as u32,
                    &mut bytes_read,
                    null_mut(),
                )
            };
            if ok == 0 {
                break;
            }
            if bytes_read == 0 {
                continue;
            }
            match decode_pipe_message(&buf[..bytes_read as usize]) {
                Ok(event) => {
                    if sender.send(event).is_err() {
                        break;
                    }
                }
                Err(err) => eprintln!("warning: dropping malformed pipe event: {err}"),
            }
        }
    }

    unsafe extern "system" fn event_record_callback(record: *mut EVENT_RECORD) {
        if let Some(event) = decode_etw_record(record) {
            if let Some(lock) = EVENT_SENDER.get() {
                if let Ok(sender) = lock.lock() {
                    if let Some(sender) = sender.as_ref() {
                        let _ = sender.send(event);
                    }
                }
            }
        }
    }

    fn decode_etw_record(record: *mut EVENT_RECORD) -> Option<TiEvent> {
        let record = unsafe { record.as_ref()? };
        if !guid_eq(&record.EventHeader.ProviderId, &ETW_TI_PROVIDER) {
            return None;
        }
        let descriptor = record.EventHeader.EventDescriptor;
        let fields = decode_tdh_fields(record as *const EVENT_RECORD);
        let name = keyword_name(descriptor.Task, descriptor.Keyword, "");

        Some(TiEvent {
            ts: record.EventHeader.TimeStamp as u64,
            event_id: descriptor.Id,
            task: descriptor.Task,
            keyword: descriptor.Keyword,
            pid: record.EventHeader.ProcessId,
            tid: record.EventHeader.ThreadId,
            name,
            fields,
        })
    }

    fn decode_tdh_fields(record: *const EVENT_RECORD) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        let mut info_size = 0;
        unsafe {
            TdhGetEventInformation(record, 0, null(), null_mut(), &mut info_size);
        }
        if info_size == 0 || info_size > 65_536 {
            return fields;
        }

        let mut info_buf = vec![0u8; info_size as usize];
        let info = info_buf.as_mut_ptr() as *mut TRACE_EVENT_INFO;
        let status = unsafe { TdhGetEventInformation(record, 0, null(), info, &mut info_size) };
        if status != ERROR_SUCCESS {
            return fields;
        }

        let property_count = unsafe { (*info).TopLevelPropertyCount };
        let property_base = unsafe { (*info).EventPropertyInfoArray.as_ptr() };
        for index in 0..property_count {
            let property = unsafe { *property_base.add(index as usize) };
            if property.Flags & PropertyStruct != 0 {
                continue;
            }
            let name_ptr =
                unsafe { (info as *const u8).add(property.NameOffset as usize) as *const u16 };
            let name = unsafe { wide_ptr_to_string(name_ptr) };
            if name.is_empty() {
                continue;
            }

            let mut descriptor = PROPERTY_DATA_DESCRIPTOR {
                PropertyName: name_ptr as u64,
                ArrayIndex: u32::MAX,
                Reserved: 0,
            };
            let mut property_size = 0;
            let size_status = unsafe {
                TdhGetPropertySize(record, 0, null(), 1, &mut descriptor, &mut property_size)
            };
            if size_status != ERROR_SUCCESS || property_size == 0 || property_size > 65_536 {
                continue;
            }

            let mut value = vec![0u8; property_size as usize + 2];
            let value_status = unsafe {
                TdhGetProperty(
                    record,
                    0,
                    null(),
                    1,
                    &descriptor,
                    property_size,
                    value.as_mut_ptr(),
                )
            };
            if value_status != ERROR_SUCCESS {
                continue;
            }
            fields.insert(
                name,
                format_tdh_value(&property, &value[..property_size as usize]),
            );
        }
        fields
    }

    fn format_tdh_value(property: &EVENT_PROPERTY_INFO, data: &[u8]) -> String {
        let in_type = unsafe { property.Anonymous1.nonStructType.InType as i32 };
        match in_type {
            TDH_INTYPE_UINT8 if data.len() >= 1 => data[0].to_string(),
            TDH_INTYPE_INT8 if data.len() >= 1 => (data[0] as i8).to_string(),
            TDH_INTYPE_UINT16 if data.len() >= 2 => {
                u16::from_le_bytes([data[0], data[1]]).to_string()
            }
            TDH_INTYPE_INT16 if data.len() >= 2 => {
                i16::from_le_bytes([data[0], data[1]]).to_string()
            }
            TDH_INTYPE_UINT32 if data.len() >= 4 => {
                u32::from_le_bytes(data[..4].try_into().expect("u32 field")).to_string()
            }
            TDH_INTYPE_INT32 if data.len() >= 4 => {
                i32::from_le_bytes(data[..4].try_into().expect("i32 field")).to_string()
            }
            TDH_INTYPE_UINT64 | TDH_INTYPE_FILETIME if data.len() >= 8 => {
                u64::from_le_bytes(data[..8].try_into().expect("u64 field")).to_string()
            }
            TDH_INTYPE_INT64 if data.len() >= 8 => {
                i64::from_le_bytes(data[..8].try_into().expect("i64 field")).to_string()
            }
            TDH_INTYPE_HEXINT32 if data.len() >= 4 => {
                format!(
                    "0x{:08X}",
                    u32::from_le_bytes(data[..4].try_into().expect("hex32 field"))
                )
            }
            TDH_INTYPE_HEXINT64 | TDH_INTYPE_POINTER if data.len() >= 8 => {
                format!(
                    "0x{:016X}",
                    u64::from_le_bytes(data[..8].try_into().expect("hex64 field"))
                )
            }
            TDH_INTYPE_BOOLEAN if data.len() >= 4 => {
                (u32::from_le_bytes(data[..4].try_into().expect("bool field")) != 0).to_string()
            }
            TDH_INTYPE_GUID if data.len() >= 16 => data
                .iter()
                .take(16)
                .map(|byte| format!("{byte:02X}"))
                .collect::<Vec<_>>()
                .join(""),
            TDH_INTYPE_UNICODESTRING => {
                let words = data
                    .chunks_exact(2)
                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                    .take_while(|word| *word != 0)
                    .collect::<Vec<_>>();
                String::from_utf16_lossy(&words)
            }
            TDH_INTYPE_ANSISTRING => {
                let end = data
                    .iter()
                    .position(|byte| *byte == 0)
                    .unwrap_or(data.len());
                String::from_utf8_lossy(&data[..end]).into_owned()
            }
            _ => data
                .iter()
                .take(16)
                .map(|byte| format!("{byte:02X}"))
                .collect::<Vec<_>>()
                .join(" "),
        }
    }

    #[derive(Default)]
    struct ProcessResolver {
        cache: HashMap<u32, String>,
    }

    impl ProcessResolver {
        fn resolve(&mut self, pid: u32) -> String {
            if let Some(name) = self.cache.get(&pid) {
                return name.clone();
            }
            let name = query_process_name(pid).unwrap_or_else(|| format!("pid:{pid}"));
            self.cache.insert(pid, name.clone());
            name
        }
    }

    fn query_process_name(pid: u32) -> Option<String> {
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle == null_mut() {
            return None;
        }
        let mut buf = vec![0u16; 32768];
        let mut len = buf.len() as u32;
        let ok = unsafe { QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len) };
        unsafe {
            CloseHandle(handle);
        }
        if ok == 0 || len == 0 {
            return None;
        }
        let full = String::from_utf16_lossy(&buf[..len as usize]);
        Some(
            full.rsplit(['\\', '/'])
                .next()
                .unwrap_or(full.as_str())
                .to_string(),
        )
    }

    fn clear_event_sender() {
        if let Some(lock) = EVENT_SENDER.get() {
            if let Ok(mut sender) = lock.lock() {
                *sender = None;
            }
        }
    }

    fn wide(input: &str) -> Vec<u16> {
        OsStr::new(input).encode_wide().chain(Some(0)).collect()
    }

    unsafe fn wide_ptr_to_string(ptr: *const u16) -> String {
        if ptr.is_null() {
            return String::new();
        }
        let mut len = 0;
        while unsafe { *ptr.add(len) } != 0 {
            len += 1;
        }
        String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(ptr, len) })
    }

    fn guid_eq(left: &GUID, right: &GUID) -> bool {
        left.data1 == right.data1
            && left.data2 == right.data2
            && left.data3 == right.data3
            && left.data4 == right.data4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_requested_keyword_mask() {
        let mask = parse_keyword_mask(
            "ALLOCVM_REMOTE,PROTECTVM_REMOTE,WRITEVM_REMOTE,READVM_REMOTE,SETTHREADCONTEXT_REMOTE",
        )
        .unwrap();
        assert_eq!(mask, 0x4 | 0x40 | 0x8_0000 | 0x2_0000 | 0x4000);
    }

    #[test]
    fn parses_hex_keyword_mask() {
        assert_eq!(parse_keyword_mask("0x4,0x40").unwrap(), 0x44);
    }

    #[test]
    fn rejects_unknown_keyword() {
        assert_eq!(
            parse_keyword_mask("NOT_A_KEYWORD").unwrap_err(),
            ConsumerError::UnknownKeyword("NOT_A_KEYWORD".to_string())
        );
    }

    #[test]
    fn derives_keyword_name_from_task_and_bit() {
        assert_eq!(keyword_name(7, 0x8_0000, ""), "WRITEVM_REMOTE");
    }

    #[test]
    fn process_filter_matches_case_insensitive_basename() {
        let filter = ProcessFilter::for_name("eqgame.exe");
        assert!(filter.matches_process_name(r"C:\Games\EverQuest\EQGAME.EXE"));
    }

    #[test]
    fn disabled_process_filter_matches_anything() {
        assert!(ProcessFilter::disabled().matches_process_name("other.exe"));
    }

    #[test]
    fn parses_fields_blob_key_value_pairs() {
        let fields = parse_fields_blob(b"SourcePid=10\0TargetPid=20\0FlagOnly\0").unwrap();
        assert_eq!(fields.get("SourcePid").unwrap(), "10");
        assert_eq!(fields.get("TargetPid").unwrap(), "20");
        assert_eq!(fields.get("FlagOnly").unwrap(), "");
    }

    #[test]
    fn decodes_current_pipe_message() {
        let msg = current_pipe_message(
            42,
            7,
            0x8_0000,
            1234,
            55,
            99,
            "WRITEVM_REMOTE",
            b"Address=0x1000\0Size=4\0",
        );
        let event = decode_pipe_message(&msg).unwrap();
        assert_eq!(event.event_id, 42);
        assert_eq!(event.task, 7);
        assert_eq!(event.keyword, 0x8_0000);
        assert_eq!(event.pid, 1234);
        assert_eq!(event.tid, 55);
        assert_eq!(event.ts, 99);
        assert_eq!(event.name, "WRITEVM_REMOTE");
        assert_eq!(event.fields.get("Size").unwrap(), "4");
    }

    #[test]
    fn decodes_legacy_pipe_message_from_issue_wire_format() {
        let msg = legacy_pipe_message(9, 100, 200, 300, "PROCESS_CREATE", b"Image=eqgame.exe\0");
        let event = decode_pipe_message(&msg).unwrap();
        assert_eq!(event.event_id, 9);
        assert_eq!(event.pid, 100);
        assert_eq!(event.tid, 200);
        assert_eq!(event.ts, 300);
        assert_eq!(event.name, "PROCESS_CREATE");
        assert_eq!(event.fields.get("Image").unwrap(), "eqgame.exe");
    }

    #[test]
    fn rejects_bad_pipe_magic() {
        let mut msg = legacy_pipe_message(1, 2, 3, 4, "X", b"");
        msg[0] = 0;
        assert_eq!(
            decode_pipe_message(&msg).unwrap_err(),
            ConsumerError::BadMagic(0x5449_5400)
        );
    }

    #[test]
    fn rejects_truncated_pipe_payload() {
        let mut msg = legacy_pipe_message(1, 2, 3, 4, "X", b"Field=Value\0");
        msg.truncate(PIPE_HEADER_LEGACY_LEN);
        assert!(matches!(
            decode_pipe_message(&msg).unwrap_err(),
            ConsumerError::TruncatedPayload { .. }
        ));
    }

    #[test]
    fn serializes_jsonl_shape() {
        let mut fields = BTreeMap::new();
        fields.insert("Address".to_string(), "0x1000".to_string());
        let event = TiEvent {
            ts: 10,
            event_id: 1,
            task: 1,
            keyword: 0x4,
            pid: 99,
            tid: 7,
            name: String::new(),
            fields,
        };
        let line = event_to_jsonl(&event, "eqgame.exe").unwrap();
        assert_eq!(
            line,
            r#"{"ts":10,"keyword":"ALLOCVM_REMOTE","pid":99,"process":"eqgame.exe","fields":{"Address":"0x1000"}}"#
        );
    }

    #[test]
    fn parses_cli_process_alias_and_keywords() {
        let options = parse_cli_args([
            "--process-name",
            "eqgame.exe",
            "--keywords",
            "ALLOCVM_REMOTE",
        ])
        .unwrap();
        assert_eq!(options.process_name, "eqgame.exe");
        assert_eq!(options.keyword_mask, 0x4);
    }

    #[allow(clippy::too_many_arguments)]
    fn current_pipe_message(
        event_id: u16,
        task: u16,
        keyword: u64,
        pid: u32,
        tid: u32,
        ts: u64,
        name: &str,
        fields: &[u8],
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&PIPE_EVENT_MAGIC.to_le_bytes());
        bytes.extend_from_slice(&event_id.to_le_bytes());
        bytes.extend_from_slice(&task.to_le_bytes());
        bytes.extend_from_slice(&keyword.to_le_bytes());
        bytes.extend_from_slice(&pid.to_le_bytes());
        bytes.extend_from_slice(&tid.to_le_bytes());
        bytes.extend_from_slice(&ts.to_le_bytes());
        bytes.extend_from_slice(&(name.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&(fields.len() as u16).to_le_bytes());
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(fields);
        bytes
    }

    fn legacy_pipe_message(
        event_id: u16,
        pid: u32,
        tid: u32,
        ts: u64,
        name: &str,
        fields: &[u8],
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&PIPE_EVENT_MAGIC.to_le_bytes());
        bytes.extend_from_slice(&event_id.to_le_bytes());
        bytes.extend_from_slice(&pid.to_le_bytes());
        bytes.extend_from_slice(&tid.to_le_bytes());
        bytes.extend_from_slice(&ts.to_le_bytes());
        bytes.extend_from_slice(&(name.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&(fields.len() as u16).to_le_bytes());
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(fields);
        bytes
    }
}
