# EQ VM Detection — Research & Evasion Catalog

**Issue**: #724  
**Status**: Research complete — implementation guidance below  
**Confidence**: Medium (string-anchored Ghidra inference; live disassembly not yet captured)

---

## 1. Discovery Anchor

String at `0x1408c6da0` in `eqgame.exe`:

```
"(AVATAR) This client is %srunning one of the detected Virtual Machines. Type: %d Flags: %d"
```

This string is the primary xref target. The function that references it (`CheckVirtualMachine` — working name) evaluates a set of VM indicators and reports two values to the server:

- **Type** — which VM platform was detected (integer enum)
- **Flags** — bitmask of which detection methods triggered

The `%s` prefix (empty string or `"not "`) indicates the check resolves to a boolean, but both the true and false cases are reported server-side.

---

## 2. Inferred Type Enum

Based on standard VM detection patterns and the platforms EQ is known to flag in forum reports:

| Type | Platform      | Primary CPUID Leaf                               | Registry/ACPI Signal                                       |
| ---- | ------------- | ------------------------------------------------ | ---------------------------------------------------------- |
| `1`  | VMware        | `EAX=0x40000000` → `VMwareVMware`                | `HKLM\SOFTWARE\VMware, Inc.\VMware Tools`                  |
| `2`  | VirtualBox    | `EAX=0x40000000` → `VBoxVBoxVBox`                | `HKLM\SOFTWARE\Oracle\VirtualBox Guest Additions`          |
| `3`  | Hyper-V       | `EAX=0x40000000` → `Microsoft Hv`                | `HKLM\SOFTWARE\Microsoft\Virtual Machine\Guest\Parameters` |
| `4`  | KVM/QEMU      | `EAX=0x40000000` → `KVMKVMKVM` or `TCGTCGTCGTCG` | ACPI tables contain `QEMU` or `BOCHS`                      |
| `5`  | Xen           | `EAX=0x40000000` → `XenVMMXenVMM`                |                                                            |
| `0`  | None detected | —                                                | —                                                          |

Type values are inferred; ground-truth assignment requires live Ghidra trace. File a sub-issue to confirm via `CheckVirtualMachine` xref walk.

---

## 3. Inferred Flags Bitmask

Each detection method that fires sets a bit. Likely mapping:

| Bit    | Method                 | Details                                                                 |
| ------ | ---------------------- | ----------------------------------------------------------------------- |
| `0x01` | CPUID hypervisor bit   | `CPUID EAX=1`: ECX bit 31 set → hypervisor present                      |
| `0x02` | CPUID vendor string    | `CPUID EAX=0x40000000`: EBX/ECX/EDX encode platform name                |
| `0x04` | Registry key presence  | Platform-specific registry keys (see Type table above)                  |
| `0x08` | MAC address prefix     | VMware `00:0C:29`, VirtualBox `08:00:27`, Hyper-V `00:15:5D`            |
| `0x10` | BIOS/ACPI strings      | DMI strings: `VBOX`, `VMWARE`, `BOCHS`, `QEMU`, `Microsoft Corporation` |
| `0x20` | Timing anomaly         | `RDTSC` delta during CPUID is unusually large on emulated hardware      |
| `0x40` | Driver/module presence | `vboxguest.sys`, `vmhgfs.sys`, `vmci.sys`, `hyperkbd.sys`, etc.         |
| `0x80` | Display adapter name   | `VMware SVGA`, `VirtualBox Graphics Adapter`, `Microsoft Basic Display` |

---

## 4. Server-Side Reporting

The format string confirms the result is sent server-side. Cross-referencing against the existing `SystemFingerprint` hook context (see `textquest-dll/src/hooks/fingerprint.rs`), the VM check likely runs as part of the same fingerprint packet or as a companion `AVATAR` message during client login.

**Key implication**: Both Type=0 (no VM) and Type≠0 (VM detected) are reported. The server can therefore:

1. Block clients where Type≠0
2. Flag accounts that alternate between VM and bare-metal sessions

This means evasion must suppress all flag bits to return `Type=0 Flags=0x00`, not merely reduce the flag count.

---

## 5. Detection Methods and Evasion Techniques

### 5.1 CPUID Hypervisor Bit (Flags 0x01 + 0x02)

**What EQ checks**: Calls `CPUID` with `EAX=1` and reads ECX bit 31. If set, follows up with `EAX=0x40000000` to read the hypervisor vendor string.

**Evasion options** (ordered by risk/complexity):

| Option | Technique                                        | Risk   | Notes                                                                                                                         |
| ------ | ------------------------------------------------ | ------ | ----------------------------------------------------------------------------------------------------------------------------- |
| A      | Hide CPUID via hypervisor config                 | Low    | VMware/VirtualBox/KVM all have `cpuid.hypervisorPresent=FALSE` or equivalent. Suppresses bit 31 and zeroes the vendor string. |
| B      | Intercept `CPUID` in DLL via hardware breakpoint | Medium | Set HWBP on `CPUID` opcode (DR0–DR3), patch ECX before return. Fragile under thread scheduling.                               |
| C      | Bare metal only                                  | None   | Eliminates the category entirely. Preferred for live TLP.                                                                     |

**Recommendation**: Use hypervisor config flags (Option A) for dev/test VMs. Bare metal (Option C) for live accounts.

### 5.2 Registry Keys (Flags 0x04)

**What EQ checks**: `RegOpenKeyEx` / `RegQueryValueEx` on platform-specific keys.

**Evasion**: Uninstall VM guest tools before running EQ. On VMware: remove VMware Tools; on VirtualBox: remove Guest Additions. Registry keys are absent when tools are not installed.

If tools are needed for clipboard/folder sharing, use a separate VM snapshot without tools for EQ sessions.

### 5.3 MAC Address Prefix (Flags 0x08)

**What EQ checks**: Enumerates network adapters via `GetAdaptersInfo` or `GetAdaptersAddresses` and checks the OUI (first 3 bytes) against known VM prefixes.

**Evasion**: Change the MAC address in the VM NIC settings to a real consumer OUI:

- Intel: `00:1A:92`, `8C:8D:28`
- Realtek: `00:E0:4C`
- Most hypervisors support MAC override in NIC settings.

### 5.4 BIOS/ACPI Strings (Flags 0x10)

**What EQ checks**: Reads DMI/SMBIOS strings via `GetSystemFirmwareTable("RSMB", 0)` or WMI `Win32_BIOS`. Looks for `VBOX`, `VMWARE`, `BOCHS`, `QEMU`, `Microsoft Corporation` in vendor/product fields.

**Evasion**:

- VMware: Set `SMBIOS.reflectHost = TRUE` in `.vmx` or use custom DMI strings (`bios.vendor`, `bios.version`, `board.manufacturer`)
- VirtualBox: `--bioslogofadein`, `--bios-system-time-offset` are insufficient; use `VBoxManage setextradata` to override DMI strings
- KVM/QEMU: `-smbios type=0,vendor="American Megatrends",version="F10"` etc.

### 5.5 Timing Anomaly (Flags 0x20)

**What EQ checks**: `RDTSC` measurements around CPUID or other operations. VM exits and emulation introduce latency that creates anomalous deltas (typically >10x native).

**Evasion**:

- Enable hardware virtualization with nested page tables (Intel EPT / AMD RVI) — reduces RDTSC overhead significantly
- Disable TSC offsetting in hypervisor (pass-through TSC mode): VMware `monitor_control.pseudo_perfctr=TRUE`, KVM `tsc-frequency` passthrough
- Bare metal eliminates this entirely

### 5.6 Driver/Module Presence (Flags 0x40)

**What EQ checks**: `CreateToolhelp32Snapshot(TH32CS_SNAPMODULE)` or `EnumDeviceDrivers` for VM-specific kernel drivers.

**Evasion**: Uninstall guest tools (same as 5.2). Drivers only present when guest additions installed. Verify with `driverquery | findstr -i vbox vmware`.

### 5.7 Display Adapter (Flags 0x80)

**What EQ checks**: `EnumDisplayDevices` or WMI `Win32_VideoController` for VM display adapter names.

**Evasion**: Install a GPU passthrough (PCI-e passthrough) or use a hypervisor with a real adapter exposed. Alternatively, a physical machine with a real GPU eliminates this.

---

## 6. Recommended Deployment Strategy

### Live TLP Accounts (36 clients)

**Recommendation: Bare metal with Frostreaver or equivalent Windows machine.**

Rationale:

- Eliminates CPUID bit, BIOS strings, timing anomaly, and driver/module vectors simultaneously
- Only MAC and registry checks remain, both trivially clean on bare metal
- No ongoing maintenance burden for VM evasion config

### Dev/Test Environment

**Minimum viable VM hardening checklist:**

1. Uninstall VMware Tools / VirtualBox Guest Additions
2. Set custom DMI strings (vendor, product, version) to real-hardware values
3. Override NIC MAC to a consumer OUI
4. Enable hardware virtualization (no software emulation)
5. Pass-through TSC mode to normalize RDTSC timing
6. Verify with: `wmic bios get Manufacturer,Version`, `getmac`, `driverquery | findstr vbox`

### TextQuest DLL Gap

The existing `SystemFingerprint` hook (`textquest-dll/src/hooks/fingerprint.rs`) already spoofs `VideoCardId`, `NetworkCardId`, `HardriveId`, and `ComputerName`. It does **not** cover the VM detection function that generates the `Type` and `Flags` values.

If EQ sends a separate AVATAR packet for the VM check (distinct from `SystemFingerprint`), a second hook is needed. This requires:

1. Ghidra xref walk from `0x1408c6da0` to identify the function entry point
2. Determine if the result is stored in a struct field or sent inline
3. Hook the function to force `Type=0, Flags=0x00` return

This is out of scope for this research pass. File a sub-issue with `agent:gemini-flash` tag to implement the `CheckVirtualMachine` hook once the function is confirmed via Ghidra.

---

## 7. Evidence Gaps and Follow-Up Issues

| Gap                                                         | Required Action                                       | Priority |
| ----------------------------------------------------------- | ----------------------------------------------------- | -------- |
| Confirm `CheckVirtualMachine` entry point                   | Ghidra xref from `0x1408c6da0`                        | High     |
| Confirm Type enum values (1–5)                              | Step through with live Ghidra / x64dbg on test client | High     |
| Confirm Flags bitmask mapping                               | Same as above                                         | Medium   |
| Determine if VM check is in `SystemFingerprint` or separate | Packet capture comparison (VM vs bare metal)          | High     |
| Implement `CheckVirtualMachine` hook if needed              | Sub-issue after above confirmed                       | Medium   |

---

## 8. References

- `textquest-dll/src/hooks/fingerprint.rs` — existing hardware fingerprint spoof
- `docs/research/hook-detection-surface.md` — hook migration priority, `SystemFingerprint` listed
- `docs/external-research/daybreak-detection-digest.md` — M5 anti-cheat exposure categories
- `docs/etw-ti-keyword-map.md` — ETW-TI evasion tactics context
- EQ string anchor: `eqgame.exe+0x1408c6da0` — `"(AVATAR) This client is %srunning one of the detected Virtual Machines. Type: %d Flags: %d"`
