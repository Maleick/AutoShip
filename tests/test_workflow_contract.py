from __future__ import annotations

"""Contract tests for the CI workflow rationalization end state.

This module intentionally encodes the target post-rationalization shape and is
expected to stay red until later workflow tasks remove the maintenance files
and simplify ci.yml.
"""

import json
from pathlib import Path
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = REPO_ROOT / ".github" / "workflows"
FEATURE_LIST = REPO_ROOT / "feature-list.json"
REMOVED_WORKFLOWS = (
    "fmt-autofix.yml",
    "branch-cleanup.yml",
    "cleanup-branches.yml",
    "auto-branch-cleanup.yml",
    "readme-metrics.yml",
    "copilot-ci-dispatch.yml",
)
FORBIDDEN_CI_STRINGS = (
    "Wait for runner-specific gate result",
    "listJobsForWorkflowRunAttempt",
    "listJobsForWorkflowRun",
    "PR gate (trusted path)",
    "PR gate (fork PR path)",
    "run_release_build",
    "  clippy_autofix:",
    "  coverage:",
    "  cargo_audit:",
    "  unsafe_code_report:",
    "  cargo_deny:",
    "  windows:",
)


class WorkflowContractTests(unittest.TestCase):
    @staticmethod
    def _job_block(text: str, job_name: str) -> list[str]:
        lines = text.splitlines()
        start = lines.index(f"  {job_name}:")
        block: list[str] = []
        for line in lines[start + 1 :]:
            if line.startswith("  ") and not line.startswith("    "):
                break
            block.append(line)
        return block

    def test_removed_maintenance_workflows_are_absent(self) -> None:
        for name in REMOVED_WORKFLOWS:
            with self.subTest(name=name):
                self.assertFalse((WORKFLOWS / name).exists(), f"{name} should be removed")

    def test_ci_workflow_shape(self) -> None:
        text = (WORKFLOWS / "ci.yml").read_text(encoding="utf-8")
        merge_gate = self._job_block(text, "merge_gate")
        secrets_scan = self._job_block(text, "secrets_scan")
        advisory_checks = self._job_block(text, "advisory_checks")
        test_matrix = self._job_block(text, "test-matrix")

        self.assertIn("    runs-on: [self-hosted, Linux, X64, textquest]", merge_gate)
        self.assertIn(
            "    if: github.event_name != 'pull_request' || github.event.pull_request.head.repo.fork == false",
            merge_gate,
        )
        self.assertNotIn("        run: cargo fmt --all --check", merge_gate)
        self.assertIn("    name: Secret scan", secrets_scan)
        self.assertIn(
            "    if: github.event_name != 'pull_request' || github.event.pull_request.head.repo.fork == false",
            secrets_scan,
        )
        self.assertIn("    name: Advisory dependency checks (manual)", advisory_checks)
        self.assertIn("    if: github.event_name == 'workflow_dispatch'", advisory_checks)
        self.assertIn("    continue-on-error: true", advisory_checks)
        self.assertIn(
            "    if: github.event_name != 'pull_request' || github.event.pull_request.head.repo.fork == false",
            test_matrix,
        )
        self.assertIn("      - name: Setup Rust (Windows)", test_matrix)
        self.assertIn("        if: runner.os == 'Windows'", test_matrix)
        self.assertIn("        uses: ./.github/actions/setup-rust-toolchain-windows", test_matrix)
        self.assertIn("      - name: Setup Rust (Linux)", test_matrix)
        self.assertIn("        if: runner.os != 'Windows'", test_matrix)
        self.assertNotIn("\n  windows:\n", text)
        for forbidden in FORBIDDEN_CI_STRINGS:
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, text)

    def test_ci_docs_only_allowlist_includes_docs_contract_tests(self) -> None:
        text = (WORKFLOWS / "ci.yml").read_text(encoding="utf-8")

        self.assertIn(
            "docs/**|site/**|*.md|*.txt|.github/workflows/docs-pages.yml|tests/test_*docs*.py|tests/test_workflow_contract.py",
            text,
        )

    def test_nightly_release_still_has_dispatch_and_schedule(self) -> None:
        text = (WORKFLOWS / "nightly-release.yml").read_text(encoding="utf-8")

        self.assertIn("workflow_dispatch", text)
        self.assertIn("schedule:", text)
        self.assertIn("build-nightly:", text)
        self.assertIn("Weekly/manual Windows validation", text)
        self.assertIn("Create or update rolling nightly prerelease", text)
        self.assertIn("runs-on: [self-hosted, Windows, X64, textquest]", text)

    def test_feature_list_includes_ci_workflow_rationalization(self) -> None:
        data = json.loads(FEATURE_LIST.read_text(encoding="utf-8"))
        features = data["features"]

        self.assertTrue(
            any(feature.get("id") == "ci-workflow-rationalization" for feature in features),
            "feature-list.json must include ci-workflow-rationalization",
        )

    def test_windows_rust_setup_action_bootstraps_rustup(self) -> None:
        text = (
            REPO_ROOT / ".github" / "actions" / "setup-rust-toolchain-windows" / "action.yml"
        ).read_text(encoding="utf-8")

        self.assertIn("shell: powershell", text)
        self.assertIn("rustup.exe not found; bootstrapping rustup from the official installer.", text)
        self.assertIn("https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe", text)
        self.assertIn('throw "rustup.exe still not found at $rustup after bootstrap."', text)

    def test_release_nightly_setup_action_bootstraps_rustup(self) -> None:
        text = (REPO_ROOT / ".github" / "actions" / "setup-rust-nightly" / "action.yml").read_text(
            encoding="utf-8"
        )

        self.assertIn("shell: powershell", text)
        self.assertIn("rustup.exe not found; bootstrapping rustup from the official installer.", text)
        self.assertIn("https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe", text)
        self.assertIn('Write-Host "[info] Nightly compiler confirmed for $env:NIGHTLY_TOOLCHAIN."', text)


if __name__ == "__main__":
    unittest.main()
