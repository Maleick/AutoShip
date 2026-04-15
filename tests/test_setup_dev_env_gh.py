import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]


class SetupDevEnvGitHubCliTests(unittest.TestCase):
    def test_full_profile_gh_check_verifies_auth_status(self) -> None:
        text = (REPO_ROOT / "scripts" / "setup-dev-env.sh").read_text(encoding="utf-8")

        self.assertIn('header "8. GitHub CLI (gh)"', text)
        self.assertIn("gh auth status", text)

    def test_full_profile_gh_check_accepts_token_fallback(self) -> None:
        text = (REPO_ROOT / "scripts" / "setup-dev-env.sh").read_text(encoding="utf-8")

        self.assertTrue(
            "GH_TOKEN" in text or "GITHUB_TOKEN" in text,
            "setup-dev-env.sh should document or check a token fallback for gh auth",
        )
        self.assertIn("GH_CONFIG_DIR", text)


if __name__ == "__main__":
    unittest.main()
