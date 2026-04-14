from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import sys
import unittest
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "dev-preflight.py"


def load_module():
    spec = importlib.util.spec_from_file_location("dev_preflight", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load dev-preflight module")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class DevPreflightTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.module = load_module()

    def test_check_command_records_missing_tool_on_path(self) -> None:
        results = []
        with mock.patch.object(self.module, "command_path", return_value=None):
            ok = self.module.check_command(results, "Git", "git", "--version", fix="Install Git")

        self.assertFalse(ok)
        self.assertEqual(len(results), 1)
        self.assertEqual(results[0].status, "FAIL")
        self.assertIn("git", results[0].detail)
        self.assertEqual(results[0].fix, "Install Git")

    def test_check_reference_trees_requires_complete_reference_files(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir)
            (repo_root / "third_party" / "eqlib").mkdir(parents=True)
            results = []

            with mock.patch.object(self.module, "REPO_ROOT", repo_root):
                self.module.check_reference_trees(results, require_reference_trees=True)

        self.assertEqual(len(results), 1)
        self.assertEqual(results[0].status, "FAIL")
        self.assertIn("incomplete", results[0].detail)
        self.assertIn("repopulate", (results[0].fix or "").lower())

    def _detect_windows_toolchain(self, active_toolchain: str) -> list:
        results = []
        with tempfile.TemporaryDirectory() as tmpdir:
            libclang_dir = Path(tmpdir)

            class FakePath:
                def __init__(self, *parts: object) -> None:
                    self.parts = tuple(str(part) for part in parts if str(part))

                def __truediv__(self, other: object) -> "FakePath":
                    return FakePath(*self.parts, other)

                def exists(self) -> bool:
                    return False

            def fake_command_path(name: str) -> str | None:
                if name == "rustup":
                    return "/opt/rustup"
                if name in {"cl", "cl.exe"}:
                    return f"/opt/{name}"
                return None

            def fake_run_command(*args: str) -> subprocess.CompletedProcess[str]:
                if args == ("rustc", "--version"):
                    return subprocess.CompletedProcess(args, 0, stdout="rustc 1.80.0\n", stderr="")
                if args == ("rustup", "show", "active-toolchain"):
                    return subprocess.CompletedProcess(args, 0, stdout=f"{active_toolchain}\n", stderr="")
                raise AssertionError(f"unexpected command: {args!r}")

            with mock.patch.object(self.module, "command_path", side_effect=fake_command_path):
                with mock.patch.object(self.module, "run_command", side_effect=fake_run_command):
                    with mock.patch.object(self.module.os, "name", "nt"):
                        with mock.patch.object(self.module.pathlib, "Path", FakePath):
                            with mock.patch.dict(self.module.os.environ, {"LIBCLANG_PATH": str(libclang_dir)}, clear=False):
                                self.module.detect_windows_toolchain(results)

        return results

    def test_detect_windows_toolchain_passes_for_nightly(self) -> None:
        results = self._detect_windows_toolchain("nightly-x86_64-pc-windows-msvc")
        self.assertEqual(results[0].status, "PASS")
        self.assertEqual(results[0].name, "Windows Rust toolchain")

    def test_detect_windows_toolchain_fails_for_stable(self) -> None:
        results = self._detect_windows_toolchain("stable-x86_64-pc-windows-msvc")
        self.assertEqual(results[0].status, "FAIL")
        self.assertEqual(results[0].name, "Windows Rust toolchain")

    def test_check_offset_sync_records_success(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir)
            script_path = repo_root / "scripts" / "validate_offsets_sync.py"
            script_path.parent.mkdir(parents=True)
            script_path.write_text("#!/usr/bin/env python3\nprint('Offset sync OK')\n")

            results = []
            completed = subprocess.CompletedProcess(("python3", str(script_path)), 0, stdout="Offset sync OK\n", stderr="")

            with mock.patch.object(self.module, "REPO_ROOT", repo_root):
                with mock.patch.object(self.module, "run_command", return_value=completed):
                    self.module.check_offset_sync(results)

        self.assertEqual(len(results), 1)
        self.assertEqual(results[0].status, "PASS")
        self.assertEqual(results[0].name, "Offset sync")
        self.assertIn("Offset sync OK", results[0].detail)


if __name__ == "__main__":
    unittest.main()
