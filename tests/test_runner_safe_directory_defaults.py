import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]


class RunnerSafeDirectoryDefaultsTests(unittest.TestCase):
    def test_runner_setup_seeds_safe_directories_from_runner_root(self) -> None:
        text = (REPO_ROOT / "scripts" / "setup-self-hosted-runner.ps1").read_text(
            encoding="utf-8"
        )
        self.assertIn("function Get-RunnerSafeDirectoryPaths", text)
        self.assertIn('Join-Path $RunnerRoot "_work"', text)
        self.assertIn(
            "Get-RunnerSafeDirectoryPaths -RunnerRoot $RunnerRoot -RepositoryName $repoName",
            text,
        )
        self.assertNotIn(
            'Add-SafeGitDirectory -Path "C:\\actions-runner\\_work\\$repoName"',
            text,
        )


if __name__ == "__main__":
    unittest.main()
