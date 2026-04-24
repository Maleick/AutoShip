from __future__ import annotations

"""Contract tests for the CI workflow rationalization end state.

This module intentionally encodes the target post-rationalization shape and is
expected to stay red until later workflow tasks remove the maintenance files
and simplify ci.yml.
"""

import json
from pathlib import Path
import unittest

import yaml


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
    def _load_workflow(name: str) -> dict:
        return yaml.load((WORKFLOWS / name).read_text(encoding="utf-8"), Loader=yaml.BaseLoader)

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
        self.assertIn("        uses: dtolnay/rust-toolchain@stable", merge_gate)
        self.assertNotIn("        uses: dtolnay/rust-toolchain@nightly", merge_gate)
        self.assertNotIn("        run: cargo fmt --all --check", merge_gate)
        self.assertIn("    name: Secret scan", secrets_scan)
        self.assertIn(
            "    if: github.event_name != 'pull_request' || github.event.pull_request.head.repo.fork == false",
            secrets_scan,
        )
        self.assertIn("    name: Advisory dependency checks (manual)", advisory_checks)
        self.assertIn("    if: github.event_name == 'workflow_dispatch'", advisory_checks)
        self.assertIn("    continue-on-error: true", advisory_checks)
        self.assertIn("    needs: merge_gate", test_matrix)
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

    def test_workflow_events_do_not_mix_paths_and_paths_ignore(self) -> None:
        for workflow_path in WORKFLOWS.glob("*.yml"):
            workflow = yaml.load(workflow_path.read_text(encoding="utf-8"), Loader=yaml.BaseLoader)
            events = workflow.get("on", {})
            if not isinstance(events, dict):
                continue
            for event_name, event_config in events.items():
                with self.subTest(workflow=workflow_path.name, event=event_name):
                    if isinstance(event_config, dict):
                        self.assertFalse(
                            {"paths", "paths-ignore"}.issubset(event_config),
                            f"{workflow_path.name}:{event_name} cannot define both paths and paths-ignore",
                        )

    def test_workflows_do_not_reference_schedule_without_trigger(self) -> None:
        for workflow_path in WORKFLOWS.glob("*.yml"):
            text = workflow_path.read_text(encoding="utf-8")
            workflow = yaml.load(text, Loader=yaml.BaseLoader)
            events = workflow.get("on", {})
            has_schedule = isinstance(events, dict) and "schedule" in events
            with self.subTest(workflow=workflow_path.name):
                if "github.event_name == 'schedule'" in text:
                    self.assertTrue(
                        has_schedule,
                        f"{workflow_path.name} references schedule but has no schedule trigger",
                    )

    def test_ci_docs_only_allowlist_excludes_map_text_files(self) -> None:
        text = (WORKFLOWS / "ci.yml").read_text(encoding="utf-8")

        self.assertIn(
            "docs/**|site/**|*.md|requirements-docs.txt|.github/workflows/docs-pages.yml|tests/test_*docs*.py|tests/test_workflow_contract.py",
            text,
        )
        self.assertIn("requirements-docs.txt", text)
        self.assertNotIn("|*.txt|", text)
        self.assertNotRegex(text, r"case \"\\$file\" in[\\s\\S]*\\*\\.txt")

    def test_ci_merge_gate_runs_one_rustfmt_check(self) -> None:
        text = (WORKFLOWS / "ci.yml").read_text(encoding="utf-8")
        merge_gate = "\n".join(self._job_block(text, "merge_gate"))
        fmt_commands = [line for line in merge_gate.splitlines() if "cargo fmt" in line]

        self.assertEqual(fmt_commands, ["        run: cargo fmt --all -- --check"])

    def test_ci_uses_deterministic_tarpaulin_install(self) -> None:
        text = (WORKFLOWS / "ci.yml").read_text(encoding="utf-8")

        self.assertNotIn("cargo install cargo-tarpaulin", text)
        self.assertNotIn("--version ^", text)
        self.assertIn("uses: taiki-e/install-action@v2", text)
        self.assertRegex(text, r"tool: cargo-tarpaulin@\d+\.\d+\.\d+")
        self.assertIn("Run coverage (threshold 68%)", text)
        self.assertIn("python3 scripts/coverage-report.py --threshold 68", text)

    def test_ci_serializes_linux_native_dependency_installs(self) -> None:
        text = (WORKFLOWS / "ci.yml").read_text(encoding="utf-8")

        self.assertEqual(text.count("flock /tmp/textquest-apt.lock"), 2)
        self.assertEqual(text.count("DPkg::Lock::Timeout=300 update"), 2)
        self.assertEqual(text.count("DPkg::Lock::Timeout=300 install -y"), 2)

    def test_ci_uses_isolated_target_dirs_without_target_cache(self) -> None:
        text = (WORKFLOWS / "ci.yml").read_text(encoding="utf-8")

        self.assertIn(
            "CARGO_TARGET_DIR: ${{ github.workspace }}/.ci-target/${{ github.run_id }}/merge-gate",
            text,
        )
        self.assertIn(
            "CARGO_TARGET_DIR: ${{ github.workspace }}/.ci-target/${{ github.run_id }}/test-${{ matrix.os-name }}-${{ matrix.rust }}",
            text,
        )
        self.assertIn(
            "CARGO_TARGET_DIR: ${{ github.workspace }}/.ci-target/${{ github.run_id }}/advisory",
            text,
        )
        self.assertIn(
            "CARGO_TARGET_DIR: ${{ github.workspace }}/.ci-target/${{ github.run_id }}/coverage",
            text,
        )
        self.assertIn('CARGO_BUILD_JOBS: "1"', text)
        self.assertEqual(text.count("cache-targets: false"), 3)

    def test_ci_installs_python_test_dependencies(self) -> None:
        text = (WORKFLOWS / "ci.yml").read_text(encoding="utf-8")

        self.assertIn("Install Python test dependencies", text)
        self.assertIn("python3 -m pip install PyYAML==6.0.3", text)

    def test_metrics_workflow_checks_static_badges_without_protected_branch_push(self) -> None:
        text = (WORKFLOWS / "metrics.yml").read_text(encoding="utf-8")
        workflow = self._load_workflow("metrics.yml")
        push = workflow["on"]["push"]

        self.assertEqual(workflow["permissions"]["contents"], "read")
        self.assertEqual(push.get("branches"), ["master"])
        self.assertNotIn("paths:", text)
        self.assertNotIn("paths-ignore:", text)
        self.assertIn(
            "if: github.event_name != 'push' || !contains(github.event.head_commit.message, '[skip metrics]')",
            text,
        )
        self.assertIn("[skip metrics]", text)
        self.assertIn("fetch-depth: 0", text)
        self.assertIn("fetch-tags: true", text)
        self.assertIn("README metrics are stale; update them in a normal PR.", text)
        self.assertNotIn("git commit", text)
        self.assertNotIn("git push", text)

    def test_pages_workflow_deploys_checked_in_site(self) -> None:
        text = (WORKFLOWS / "docs-pages.yml").read_text(encoding="utf-8")

        self.assertIn('- "site/**"', text)
        self.assertNotIn('- "docs/wiki/**"', text)
        self.assertNotIn("mkdocs build", text)
        self.assertNotIn("python3 -m pip install", text)
        self.assertIn("path: ./site", text)

    def test_publish_workflows_use_existing_package_dirs(self) -> None:
        npm_text = (WORKFLOWS / "publish-npm.yml").read_text(encoding="utf-8")
        python_text = (WORKFLOWS / "publish-python.yml").read_text(encoding="utf-8")

        self.assertIn("cd textquest-client/typescript", npm_text)
        self.assertNotIn("textquest-client/tstextquest", npm_text)
        self.assertTrue((REPO_ROOT / "textquest-client" / "typescript" / "package.json").exists())

        self.assertIn("cd textquest-client/python", python_text)
        self.assertIn("packages-dir: textquest-client/python/dist/", python_text)
        self.assertNotIn("textquest-client/pytextquest", python_text)
        self.assertTrue((REPO_ROOT / "textquest-client" / "python" / "pyproject.toml").exists())

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
