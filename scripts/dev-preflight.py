#!/usr/bin/env python3

from __future__ import annotations

import argparse
import os
import pathlib
import platform
import re
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


def check_map_files(results: list[CheckResult]) -> None:
    """Validate all map files in config/maps/*.txt."""
    import re

    maps_dir = REPO_ROOT / "config" / "maps"
    if not maps_dir.exists():
        record(results, "WARN", "Map files", "config/maps/ directory not found — skipping map validation.")
        return

    map_files = sorted(maps_dir.glob("*.txt"))
    if not map_files:
        record(results, "WARN", "Map files", "No *.txt files found in config/maps/.")
        return

    # Valid line patterns:
    #   L x1, y1, z1, x2, y2, z2, r, g, b   (line segment)
    #   P x, y, z, r, g, b, size, label       (point/label)
    #   * comment
    #   (blank lines are allowed)
    float_re = r"[-+]?\d+(?:\.\d+)?"
    int_re = r"\d+"
    line_pattern = re.compile(
        r"^L\s+"
        + r",\s*".join([float_re] * 6)
        + r",\s*"
        + r",\s*".join([int_re] * 3)
        + r"\s*$"
    )
    point_pattern = re.compile(
        r"^P\s+"
        + r",\s*".join([float_re] * 3)
        + r",\s*"
        + r",\s*".join([int_re] * 3)
        + r",\s*"
        + int_re
        + r",\s*\S.*$"
    )

    errors: list[str] = []
    empty_files: list[str] = []
    checked = 0

    for map_file in map_files:
        try:
            content = map_file.read_text(encoding="utf-8", errors="replace")
        except OSError as exc:
            errors.append(f"{map_file.name}: read error — {exc}")
            continue

        lines = content.splitlines()
        non_blank = [ln for ln in lines if ln.strip()]
        if not non_blank:
            empty_files.append(map_file.name)
            continue

        checked += 1
        for lineno, raw in enumerate(lines, start=1):
            stripped = raw.strip()
            if not stripped or stripped.startswith("*"):
                continue
            if line_pattern.match(stripped) or point_pattern.match(stripped):
                continue
            errors.append(f"{map_file.name}:{lineno}: unexpected format — {stripped[:60]!r}")

    total = len(map_files)
    if errors or empty_files:
        detail_parts: list[str] = []
        if empty_files:
            detail_parts.append(f"empty files: {', '.join(empty_files)}")
        if errors:
            detail_parts.append("; ".join(errors[:5]))
            if len(errors) > 5:
                detail_parts.append(f"…and {len(errors) - 5} more error(s)")
        record(
            results,
            "FAIL",
            "Map files",
            f"Checked {total} map file(s) — issues found: {'; '.join(detail_parts)}",
            "Ensure each map line starts with L or P (with correct field counts) or * for comments.",
        )
    else:
        record(results, "PASS", "Map files", f"{checked}/{total} map file(s) passed validation.")


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


def check_reference_trees(
    results: list[CheckResult],
    require_reference_trees: bool,
    eqlib_root: str | None = None,
    macroquest_root: str | None = None,
) -> None:
    configured_roots: list[tuple[str, pathlib.Path, list[pathlib.Path]]] = []
    if eqlib_root is None:
        default_eqlib_root = REPO_ROOT / "third_party" / "eqlib"
        if default_eqlib_root.exists():
            eqlib_root = str(default_eqlib_root)
    if macroquest_root is None:
        for candidate in (
            REPO_ROOT / "third_party" / "macroquest",
            REPO_ROOT / "third_party" / "MacroQuest",
        ):
            if candidate.exists():
                macroquest_root = str(candidate)
                break

    if eqlib_root:
        root = pathlib.Path(eqlib_root).expanduser()
        configured_roots.append(
            (
                "eqlib",
                root,
                [root / "include/eqlib/offsets/eqgame.h"],
            )
        )
    if macroquest_root:
        root = pathlib.Path(macroquest_root).expanduser()
        configured_roots.append(
            (
                "macroquest",
                root,
                [root / "src/login", root / "src/routing"],
            )
        )

    if not configured_roots:
        detail = "No optional local reference roots were configured."
        fix = (
            "Set `TEXTQUEST_EQLIB_ROOT` / `TEXTQUEST_MACROQUEST_ROOT` or pass "
            "`--eqlib-root` / `--macroquest-root` before offset or struct work."
        )
        if require_reference_trees:
            record(results, "FAIL", "Reference trees", detail, fix)
        else:
            record(results, "PASS", "Reference trees", f"{detail} Routine cargo work is still fine without them.")
        return

    missing_roots = [f"{name}={root}" for name, root, _ in configured_roots if not root.exists()]
    if missing_roots:
        detail = f"Configured reference roots are missing: {', '.join(missing_roots)}"
        fix = "Point the preflight helper at existing local eqlib or MacroQuest checkouts."
        status_name = "FAIL" if require_reference_trees else "WARN"
        record(results, status_name, "Reference trees", detail, fix)
        return

    missing_paths = [
        f"{name}:{path}"
        for name, _, expected in configured_roots
        for path in expected
        if not path.exists()
    ]
    if missing_paths:
        detail = f"Configured reference roots are present but incomplete: {', '.join(missing_paths)}"
        fix = "Refresh or repopulate the local eqlib or MacroQuest checkout before offset or struct work."
        status_name = "FAIL" if require_reference_trees else "WARN"
        record(results, status_name, "Reference trees", detail, fix)
        return

    summary = ", ".join(f"{name}={root}" for name, root, _ in configured_roots)
    profile_note = "Required reference material is available." if require_reference_trees else "Ready if you need offset or struct work."
    record(results, "PASS", "Reference trees", f"Optional local reference roots are configured ({summary}). {profile_note}")


def check_offset_sync(results: list[CheckResult]) -> None:
    script = REPO_ROOT / "scripts" / "validate_offsets_sync.py"
    if not script.exists():
        record(
            results,
            "FAIL",
            "Offset sync",
            "scripts/validate_offsets_sync.py not found.",
            "Add scripts/validate_offsets_sync.py and wire it into CI.",
        )
        return

    completed = run_command(sys.executable, str(script))
    if completed.returncode == 0:
        detail = first_line(completed.stdout or completed.stderr)
        record(results, "PASS", "Offset sync", detail)
        return

    detail = first_line(completed.stderr or completed.stdout)
    record(
        results,
        "FAIL",
        "Offset sync",
        detail,
        "Inspect the diff and reconcile config/offsets.json with textquest-common/src/offsets.rs.",
    )


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
            print(
                "Need local eqlib or MacroQuest refs too? "
                "Rerun with `--require-reference-trees --eqlib-root /path/to/eqlib "
                "--macroquest-root /path/to/macroquest` or set "
                "`TEXTQUEST_EQLIB_ROOT` / `TEXTQUEST_MACROQUEST_ROOT`."
            )
        return 0

    print("Preflight found blocking issues. Fix them and rerun this script.")
    return 1


def run_ci_step(name: str, results: list[CheckResult], args: list[str], fix: str | None = None) -> bool:
    """Run a CI-equivalent command with live output, recording pass/fail."""
    print(f"\n{'=' * 60}")
    print(f"  {name}")
    print(f"{'=' * 60}")
    completed = subprocess.run(
        args,
        cwd=REPO_ROOT,
        check=False,
    )
    if completed.returncode == 0:
        record(results, "PASS", name, "OK")
        return True
    record(results, "FAIL", name, f"Exit code {completed.returncode}", fix)
    return False


def run_ci_checks(results: list[CheckResult], *, fix: bool, skip_tests: bool, has_fmt: bool, has_clippy: bool) -> None:
    """Run local validation (fmt plus the PR gate checks).

    The order is optimised for fast local feedback (fmt first, then clippy,
    then tests), so it intentionally includes local formatting before the
    wiki/lint/test/Python checks that the PR gate enforces in ci.yml.
    """
    print("\n")
    print("=" * 60)
    print("  Running local validation (fmt + PR gate checks)")
    print("=" * 60)

    # 0. Wiki sync check (CI runs this before cargo steps)
    sync_wiki = REPO_ROOT / "scripts" / "sync_wiki.py"
    if sync_wiki.exists():
        run_ci_step("wiki sync check", results, [_python_cmd(), str(sync_wiki), "--check"])

    # 1. cargo fmt
    if has_fmt:
        if fix:
            run_ci_step("cargo fmt --all (autofix)", results, ["cargo", "fmt", "--all"])
        run_ci_step(
            "cargo fmt --all --check",
            results,
            ["cargo", "fmt", "--all", "--check"],
            fix="Run `cargo fmt --all` or rerun with `--fix`.",
        )
    else:
        record(results, "FAIL", "cargo fmt --all --check", "Skipped — rustfmt not available.",
               "Run `rustup component add rustfmt`.")

    # 2. cargo clippy
    if has_clippy:
        run_ci_step(
            "cargo clippy",
            results,
            ["cargo", "clippy", "--all-targets", "--all-features", "--", "-D", "warnings"],
        )
    else:
        record(results, "FAIL", "cargo clippy", "Skipped — clippy not available.",
               "Run `rustup component add clippy`.")

    # 3. cargo doc
    run_ci_step(
        "cargo doc",
        results,
        ["cargo", "doc", "--no-deps", "--workspace", "--all-features"],
    )

    if skip_tests:
        record(results, "PASS", "cargo test", "Skipped (--skip-tests)")
        record(results, "PASS", "Python tests", "Skipped (--skip-tests)")
        return

    # 4. cargo test
    run_ci_step("cargo test", results, ["cargo", "test"])

    # 5. Python tests
    run_ci_step(
        "Python tests",
        results,
        [_python_cmd(), "-m", "unittest", "discover", "-s", "tests", "-p", "test_*.py", "-v"],
    )


def _python_cmd() -> str:
    # Prefer the interpreter running this script so subprocesses share
    # the same virtualenv / environment as the preflight command itself.
    if sys.executable:
        return sys.executable
    for candidate in ("python3", "python"):
        if command_path(candidate):
            return candidate
    return "python3"


def update_test_count() -> None:
    """Count workspace tests and patch the ~N tests figure in CLAUDE.md."""
    result = subprocess.run(
        ["cargo", "test", "--all", "--", "--list"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
    )
    count = result.stdout.count(": test")
    if count == 0:
        print("update-docs: could not count tests (build may be needed)")
        return

    claude_md = REPO_ROOT / "CLAUDE.md"
    text = claude_md.read_text()
    updated = re.sub(r"~[\d,]+\+ tests", f"~{count:,}+ tests", text)
    if updated == text:
        print(f"update-docs: test count already current ({count:,})")
        return
    claude_md.write_text(updated)
    print(f"update-docs: CLAUDE.md test count updated to ~{count:,}")


def main() -> int:
    parser = argparse.ArgumentParser(description="TextQuest developer bootstrap/preflight helper.")
    parser.add_argument(
        "--require-reference-trees",
        action="store_true",
        help="Treat missing optional local MacroQuest/eqlib reference roots as blocking failures.",
    )
    parser.add_argument(
        "--eqlib-root",
        default=os.environ.get("TEXTQUEST_EQLIB_ROOT"),
        help="Path to a local eqlib checkout. Defaults to TEXTQUEST_EQLIB_ROOT if set.",
    )
    parser.add_argument(
        "--macroquest-root",
        default=os.environ.get("TEXTQUEST_MACROQUEST_ROOT"),
        help="Path to a local MacroQuest checkout. Defaults to TEXTQUEST_MACROQUEST_ROOT if set.",
    )
    parser.add_argument(
        "--fix",
        action="store_true",
        help="Auto-fix formatting issues (runs `cargo fmt --all` before checking).",
    )
    parser.add_argument(
        "--skip-tests",
        action="store_true",
        help="Skip cargo test and Python tests (only run fmt + clippy).",
    )
    parser.add_argument(
        "--env-only",
        action="store_true",
        help="Only check the development environment, skip CI checks.",
    )
    parser.add_argument(
        "--update-docs",
        action="store_true",
        help="Count workspace tests and patch the test count in CLAUDE.md.",
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

    has_fmt = False
    has_clippy = False
    if cargo_ok:
        fmt = run_command("cargo", "fmt", "--version")
        if fmt.returncode == 0:
            record(results, "PASS", "rustfmt", first_line(fmt.stdout or fmt.stderr))
            has_fmt = True
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
            has_clippy = True
        else:
            record(
                results,
                "WARN",
                "clippy",
                "The clippy component is not available.",
                "Run `rustup component add clippy`.",
            )

    check_map_files(results)

    check_command(
        results,
        "CMake",
        "cmake",
        "--version",
        fix="Install CMake 3.5+ and ensure `cmake` is on PATH.",
    )

    if rustc_ok:
        detect_windows_toolchain(results)

    if git_ok:
        check_reference_trees(
            results,
            require_reference_trees=args.require_reference_trees,
            eqlib_root=args.eqlib_root,
            macroquest_root=args.macroquest_root,
        )

    # Validate map files if config/maps/ exists
    map_dir = REPO_ROOT / "config" / "maps"
    if map_dir.exists():
        validate_maps_script = REPO_ROOT / "scripts" / "validate-maps.py"
        if validate_maps_script.exists():
            completed = run_command(sys.executable, str(validate_maps_script))
            if completed.returncode == 0:
                detail = first_line(completed.stdout or completed.stderr)
                record(results, "PASS", "Map validation", detail)
            else:
                detail = first_line(completed.stderr or completed.stdout)
                record(
                    results,
                    "FAIL",
                    "Map validation",
                    detail,
                    "Fix map format errors reported by scripts/validate-maps.py.",
                )
        else:
            record(results, "WARN", "Map validation", "scripts/validate-maps.py not found; skipping.")
    else:
        record(results, "PASS", "Map validation", "No config/maps/ directory; skipping.")

    check_offset_sync(results)

    if args.update_docs and cargo_ok:
        update_test_count()

    if not args.env_only:
        run_ci_checks(
            results,
            fix=args.fix,
            skip_tests=args.skip_tests,
            has_fmt=has_fmt,
            has_clippy=has_clippy,
        )

    return print_results(results, require_reference_trees=args.require_reference_trees)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        print("\nPreflight interrupted.")
        sys.exit(130)
