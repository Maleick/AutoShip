import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]


class CiRunnerWorkspacePrepTests(unittest.TestCase):
    def test_prepare_action_exists_and_cleans_workspace(self) -> None:
        text = (
            REPO_ROOT
            / ".github"
            / "actions"
            / "prepare-linux-runner-workspace"
            / "action.yml"
        ).read_text(encoding="utf-8")
        self.assertIn("Prepare Linux self-hosted workspace", text)
        self.assertIn("find \"$workspace\" -mindepth 1 -maxdepth 1 -exec rm -rf -- {} +", text)
        self.assertIn("Fix runner ownership or install passwordless sudo", text)

    def test_linux_checkout_workflows_prepare_workspace_before_checkout(self) -> None:
        checks = {
            "ci.yml": (
                REPO_ROOT / ".github" / "workflows" / "ci.yml",
                "  secrets_scan:\n",
            ),
            "claude-agent.yml": (
                REPO_ROOT / ".github" / "workflows" / "claude-agent.yml",
                "  claude:\n",
            ),
            "super-linter.yml": (
                REPO_ROOT / ".github" / "workflows" / "super-linter.yml",
                "  super-linter:\n",
            ),
        }

        for workflow_name, (path, section_marker) in checks.items():
            with self.subTest(workflow=workflow_name):
                text = path.read_text(encoding="utf-8")
                start = text.index(section_marker)
                section = text[start:]
                prep = "uses: ./.github/actions/prepare-linux-runner-workspace"
                checkout = "uses: actions/checkout@v5"
                self.assertIn(prep, section)
                self.assertIn(checkout, section)
                self.assertLess(section.index(prep), section.index(checkout))


if __name__ == "__main__":
    unittest.main()
