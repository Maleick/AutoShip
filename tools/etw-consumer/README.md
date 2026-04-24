# ETW-TI Consumer

`etw-consumer` is a Windows-only JSONL streamer for
`Microsoft-Windows-Threat-Intelligence` events plus the prebuilt
`EtwTiDriver.sys` process/image callback pipe.

```powershell
cargo run -p etw-consumer -- `
  --process eqgame.exe `
  --keywords ALLOCVM_REMOTE,PROTECTVM_REMOTE,WRITEVM_REMOTE,READVM_REMOTE,SETTHREADCONTEXT_REMOTE
```

Each matching event is written to stdout:

```json
{"ts":1337,"keyword":"WRITEVM_REMOTE","pid":1234,"process":"eqgame.exe","fields":{"Address":"0x1000"}}
```

## Requirements

- Windows 10 21H2+ or Windows 11 x64 in an isolated VM.
- Run from an elevated shell.
- `EtwTiDriver.sys` must already be built, test-signed, installed, and loaded.
- Test signing must be enabled and Secure Boot disabled for the prebuilt driver.
- The consumer process must be patched to `PS_PROTECTED_ANTIMALWARE_LIGHT`
  (`EPROCESS->Protection = 0x31`) before ETW-TI provider enablement. Without
  that patch, `EnableTraceEx2` returns access denied.

The `EPROCESS->Protection` offset changes between Windows builds. Resolve it in
WinDbg with `dt nt!_EPROCESS <process> Protection`; do not hardcode an offset.

## Sources

- ETW source: `StartTraceW`, `EnableTraceEx2`, `OpenTraceW`, `ProcessTrace`,
  and TDH property decoding.
- Pipe source: `\\.\pipe\EtwTiForwarder` binary messages from the loaded driver.
- Driver control: `\\.\EtwTiDriver` with `IOCTL_ETWTI_START` and
  `IOCTL_ETWTI_STOP`.

The parser accepts the current `EtwTiShared.h` pipe header with `Task` and
`Keyword` fields, and the older issue-body wire format without those fields.
