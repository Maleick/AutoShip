#!/usr/bin/env python3
"""Security tests for winrm_exec.py to ensure command injection is prevented."""

import base64
import sys
import unittest
from pathlib import Path

# Add scripts directory to path so we can import winrm_exec
sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))

try:
    import winrm_exec
    WINRM_AVAILABLE = True
except ImportError:
    WINRM_AVAILABLE = False


class TestWinrmExecSecurity(unittest.TestCase):
    """Test that winrm_exec properly encodes commands to prevent injection."""

    def setUp(self):
        if not WINRM_AVAILABLE:
            self.skipTest("winrm package not installed — skipping security tests")

    def _assert_encodes_round_trip(self, command):
        """Assert the encoded command decodes from base64/UTF-16LE to the original."""
        encoded = winrm_exec._encode_powershell_command(command)
        self.assertIsInstance(encoded, str)
        decoded = base64.b64decode(encoded).decode("utf-16-le")
        self.assertEqual(decoded, command)
        return encoded

    def test_encode_powershell_command_basic(self):
        """Test basic command encoding."""
        command = "Write-Host 'Hello World'"
        self._assert_encodes_round_trip(command)

    def test_encode_prevents_quote_injection(self):
        """Test that single quotes cannot break out of encoding."""
        command = "'; evil command; '"
        self._assert_encodes_round_trip(command)

    def test_encode_prevents_semicolon_injection(self):
        """Test that semicolons in commands don't execute additional commands."""
        command = "Write-Host 'test'; Remove-Item -Recurse -Force C:\\"
        self._assert_encodes_round_trip(command)

    def test_encode_prevents_variable_injection(self):
        """Test that PowerShell variables are not expanded."""
        command = "$(evil_command)"
        self._assert_encodes_round_trip(command)

    def test_encode_handles_unicode(self):
        """Test that Unicode characters are handled correctly."""
        command = "Write-Host '日本語テスト'"
        self._assert_encodes_round_trip(command)

    def test_encode_empty_command(self):
        """Test encoding of empty command."""
        self._assert_encodes_round_trip("")

    def test_encode_returns_string(self):
        """Test that encoding always returns a string."""
        result = winrm_exec._encode_powershell_command("test")
        self.assertIsInstance(result, str)

    def test_interactive_task_command_uses_encoded_payload_only(self):
        """Test that the scheduled-task wrapper stays on the encoded path."""
        encoded = self._assert_encodes_round_trip("Write-Host 'safe'")
        task_command = winrm_exec._build_interactive_task_command(encoded)

        self.assertIn("-EncodedCommand", task_command)
        self.assertNotIn("Write-Host 'safe'", task_command)
        self.assertNotIn("@echo off", task_command)
        self.assertNotIn("cmd.exe", task_command)


if __name__ == "__main__":
    unittest.main()
