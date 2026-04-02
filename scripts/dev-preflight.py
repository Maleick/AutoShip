#!/usr/bin/env python3

from __future__ import annotations

import argparse
import os
import pathlib
import platform
import shutil
import subprocess
import sys
from dataclasses import dataclass


SCRIPT_PATH = pathlib.Path(__file__).resolve()
REPO_ROOT = SCRIPT_PATH.parent.parent


@dataclass
class CheckResult:
    status: str
    name: str
    detail: str
    fix: str | None = None


def run_command(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        list(args),
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


def first_line(output: str) -> str:
    for line in output.splitlines():
        stripped = line.strip()
        if stripped:
            return stripped
    return "(no output)"


def record(results: list[CheckResult], status: str, name: str, detail: str, fix: str | None = None) -> None:
    results.append(CheckResult(status=status, name=name, detail=detail, fix=fix))


def command_path(name: str) -> str | None:
    return shutil.which(name)


def declared_submodule_paths() -> list[str]:
    config = run_command("git", "config", "-f", ".gitmodules", "--get-regexp", r"^submodule\..*\.path$")
    if config.returncode != 0:
        return []

    paths: list[str] = []
    for raw_line in config.stdout.splitlines():
        parts = raw_line.strip().split(maxsplit=1)
        if len(parts) == 2 and parts[1]:
            paths.append(parts[1].strip())
    return paths


def check_command(results: list[CheckResult], name: str, command: str, *args: str, fix: str | None = None) -> bool:
    path = command_path(command)
    if not path:
        record(results, "FAIL", name, f"`{command}` is not on PATH.", fix)
        return False

    completed = run_command(command, *args)
    if completed.returncode != 0:
        detail = first_line(completed.stderr or completed.stdout)
        record(results, "FAIL", name, detail, fix)
        return False

    detail = first_line(completed.stdout or completed.stderr)
    record(results, "PASS", name, f"{detail} ({path})")
    return True


def detect_windows_toolchain(results: list[CheckResult]) -> None:
    if os.name != "nt":
        return

    rustc = run_command("rustc", "--version")
    rustup_path = command_path("rustup")
    active_toolchain = ""
    if rustup_path:
        active = run_command("rustup", "show", "active-toolchain")
        if active.returncode == 0:
            active_toolchain = first_line(active.stdout)

    if "nightly" in active_toolchain.lower() or "nightly" in (rustc.stdout or "").lower():
        record(results, "PASS", "Windows Rust toolchain", active_toolchain or first_line(rustc.stdout))
    else:
        fix = (
            "Run `rustup toolchain install nightly-x86_64-pc-windows-msvc` and "
            "`rustup default nightly-x86_64-pc-windows-msvc`."
        )
        detail = active_toolchain or first_line(rustc.stdout or rustc.stderr)
        record(results, "FAIL", "Windows Rust toolchain", detail, fix)

    cl_path = command_path("cl.exe") or command_path("cl")
    vswhere = pathlib.Path(os.environ.get("ProgramFiles(x86)", "")) / "Microsoft Visual Studio" / "Installer" / "vswhere.exe"
    if cl_path:
        record(results, "PASS", "MSVC build tools", f"`cl.exe` available at {cl_path}")
    elif vswhere.exists():
        probe = subprocess.run(
            [
                str(vswhere),
                "-latest",
                "-products",
                "*",
                "-requires",
                "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
                "-property",
                "installationPath",
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        installation = first_line(probe.stdout)
        if probe.returncode == 0 and installation != "(no output)":
            record(results, "PASS", "MSVC build tools", installation)
        else:
            record(
                results,
                "FAIL",
                "MSVC build tools",
                "Visual Studio Build Tools were not detected.",
                "Run `scripts\\setup-windows.ps1` or install the Desktop development with C++ workload.",
            )
    else:
        record(
            results,
            "FAIL",
            "MSVC build tools",
            "Could not find `cl.exe` or `vswhere.exe`.",
            "Run `scripts\\setup-windows.ps1` or install the Desktop development with C++ workload.",
        )

    libclang_path = os.environ.get("LIBCLANG_PATH")
    if libclang_path:
        libclang_dir = pathlib.Path(libclang_path)
        if libclang_dir.exists():
            record(results, "PASS", "LLVM/libclang", f"LIBCLANG_PATH={libclang_dir}")
            return
        record(
            results,
            "WARN",
            "LLVM/libclang",
            f"LIBCLANG_PATH points to a missing directory: {libclang_dir}",
            "Install LLVM or fix LIBCLANG_PATH before Windows bindgen work.",
        )
        return

    common_llvm = pathlib.Path("C:/Program Files/LLVM/lib")
    if common_llvm.exists():
        record(results, "PASS", "LLVM/libclang", f"Found LLVM at {common_llvm}")
    else:
        record(
            results,
            "WARN",
            "LLVM/libclang",
            "LLVM was not detected. Cargo builds that invoke bindgen may fail on Windows.",
            "Install LLVM and set LIBCLANG_PATH to `C:/Program Files/LLVM/lib` if needed.",
        )


def sync_submodules(results: list[CheckResult]) -> None:
    print("Initializing submodules with `git submodule update --init --recursive`...")
    target_args = ["--", *declared_submodule_paths()]
    sync = run_command("git", "submodule", "sync", "--recursive", *target_args)
    if sync.returncode != 0:
        record(
            results,
            "FAIL",
            "Submodule sync",
            first_line(sync.stderr or sync.stdout),
            "Run `git submodule sync --recursive` manually.",
        )
        return

    update = subprocess.run(
        ["git", "submodule", "update", "--init", "--recursive", *target_args],
        cwd=REPO_ROOT,
        check=False,
    )
    if update.returncode == 0:
        record(results, "PASS", "Submodule sync", "Initialized and updated submodules recursively.")
    else:
        record(
            results,
            "FAIL",
            "Submodule sync",
            f"`git submodule update --init --recursive` exited with status {update.returncode}.",
            "Run `git submodule update --init --recursive` manually.",
        )


def check_reference_submodules(results: list[CheckResult], require_reference_trees: bool) -> None:
    if not (REPO_ROOT / ".gitmodules").exists():
        record(results, "PASS", "Reference submodules", "No `.gitmodules` file present.")
        return

    submodule_paths = declared_submodule_paths()
    if not submodule_paths:
        record(results, "PASS", "Reference submodules", "No submodule paths declared in `.gitmodules`.")
        return

    status = run_command("git", "submodule", "status", "--recursive", "--", *submodule_paths)
    if status.returncode != 0:
        record(
            results,
            "FAIL",
            "Reference submodules",
            first_line(status.stderr or status.stdout),
            "Run `git submodule update --init --recursive`.",
        )
        return

    missing: list[str] = []
    drifted: list[str] = []
    conflicted: list[str] = []
    ready = 0

    for raw_line in status.stdout.splitlines():
        if not raw_line:
            continue
        flag = raw_line[0]
        remainder = raw_line[1:].strip()
        parts = remainder.split()
        if len(parts) < 2:
            continue
        path = parts[1]
        if flag == "-":
            missing.append(path)
        elif flag == "+":
            drifted.append(path)
        elif flag == "U":
            conflicted.append(path)
        else:
            ready += 1

    required_entries = {
        "third_party/eqlib",
        "third_party/macroquest",
        "third_party/macroquest/src/eqlib",
    }
    required_paths = [
        REPO_ROOT / "third_party/eqlib/include/eqlib/offsets/eqgame.h",
        REPO_ROOT / "third_party/macroquest/src/login",
        REPO_ROOT / "third_party/macroquest/src/routing",
    ]
    missing_paths = [str(path.relative_to(REPO_ROOT)) for path in required_paths if not path.exists()]
    blocking_missing = [path for path in missing if path in required_entries]
    optional_missing = [path for path in missing if path not in required_entries]

    if conflicted:
        record(
            results,
            "FAIL",
            "Reference submodules",
            f"Merge conflicts detected in: {', '.join(conflicted)}",
            "Resolve the submodule conflicts, then rerun preflight.",
        )
        return

    if blocking_missing or missing_paths:
        missing_detail = ", ".join(blocking_missing + missing_paths)
        detail = f"Missing or uninitialized reference trees: {missing_detail}"
        fix = "Run `git submodule update --init --recursive` or rerun with `--init-submodules`."
        status_name = "FAIL" if require_reference_trees else "WARN"
        if not require_reference_trees:
            detail += ". Routine cargo work is still fine without them."
        record(results, status_name, "Reference submodules", detail, fix)
        return

    if drifted:
        record(
            results,
            "WARN",
            "Reference submodules",
            f"Submodules are checked out at non-recorded commits: {', '.join(drifted)}",
            "Run `git submodule update --init --recursive` to realign them.",
        )
        return

    if optional_missing:
        record(
            results,
            "WARN",
            "Reference submodules",
            (
                "Reference trees are ready, but extra nested MacroQuest submodules are still "
                f"missing: {', '.join(optional_missing)}"
            ),
            "Run `git submodule update --init --recursive` for a fully hydrated vendor tree.",
        )
        return

    profile_note = "Required for reference work." if require_reference_trees else "Ready if you need offset or struct work."
    record(results, "PASS", "Reference submodules", f"{ready} submodule entries are ready. {profile_note}")


def print_results(results: list[CheckResult], require_reference_trees: bool) -> int:
    counts = {"PASS": 0, "WARN": 0, "FAIL": 0}
    for result in results:
        counts[result.status] += 1
        print(f"[{result.status}] {result.name}: {result.detail}")
        if result.fix:
            print(f"        Fix: {result.fix}")

    print()
    print(
        "Summary: "
        f"{counts['PASS']} passed, {counts['WARN']} warnings, {counts['FAIL']} failed "
        f"({'reference' if require_reference_trees else 'general'} profile)"
    )

    if counts["FAIL"] == 0:
        print("Next: run `cargo build`, `cargo test`, or `cargo run`.")
        if not require_reference_trees:
            print("Need MacroQuest/eqlib refs too? Rerun with `--require-reference-trees`.")
        return 0

    print("Preflight found blocking issues. Fix them and rerun this script.")
    return 1


def main() -> int:
    parser = argparse.ArgumentParser(description="DMFT developer bootstrap/preflight helper.")
    parser.add_argument(
        "--init-submodules",
        action="store_true",
        help="Initialize or update git submodules before checking them.",
    )
    parser.add_argument(
        "--require-reference-trees",
        action="store_true",
        help="Treat missing MacroQuest/eqlib reference submodules as blocking failures.",
    )
    args = parser.parse_args()

    os.chdir(REPO_ROOT)
    results: list[CheckResult] = []

    record(results, "PASS", "Repository", str(REPO_ROOT))
    record(results, "PASS", "Platform", f"{platform.system()} {platform.release()}")

    git_ok = check_command(
        results,
        "Git",
        "git",
        "--version",
        fix="Install Git and ensure it is on PATH.",
    )
    rustc_ok = check_command(
        results,
        "Rust compiler",
        "rustc",
        "--version",
        fix="Install Rust from https://rustup.rs or run `scripts\\setup-windows.ps1` on Windows.",
    )
    cargo_ok = check_command(
        results,
        "Cargo",
        "cargo",
        "--version",
        fix="Install Rust from https://rustup.rs or run `scripts\\setup-windows.ps1` on Windows.",
    )

    if cargo_ok:
        fmt = run_command("cargo", "fmt", "--version")
        if fmt.returncode == 0:
            record(results, "PASS", "rustfmt", first_line(fmt.stdout or fmt.stderr))
        else:
            record(
                results,
                "WARN",
                "rustfmt",
                "The rustfmt component is not available.",
                "Run `rustup component add rustfmt`.",
            )

        clippy = run_command("cargo", "clippy", "--version")
        if clippy.returncode == 0:
            record(results, "PASS", "clippy", first_line(clippy.stdout or clippy.stderr))
        else:
            record(
                results,
                "WARN",
                "clippy",
                "The clippy component is not available.",
                "Run `rustup component add clippy`.",
            )

    check_command(
        results,
        "CMake",
        "cmake",
        "--version",
        fix="Install CMake 3.5+ and ensure `cmake` is on PATH.",
    )

    if rustc_ok:
        detect_windows_toolchain(results)

    if git_ok and args.init_submodules:
        sync_submodules(results)

    if git_ok:
        check_reference_submodules(results, require_reference_trees=args.require_reference_trees)

    return print_results(results, require_reference_trees=args.require_reference_trees)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        print("\nPreflight interrupted.")
        sys.exit(130)
