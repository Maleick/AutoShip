from __future__ import annotations

from pathlib import Path
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]


class RustfmtContractTests(unittest.TestCase):
    def test_repo_uses_single_root_rustfmt_config(self) -> None:
        self.assertTrue((REPO_ROOT / "rustfmt.toml").exists())
        self.assertFalse(
            (REPO_ROOT / ".rustfmt.toml").exists(),
            "rustfmt config must live only in rustfmt.toml to keep CI formatting deterministic",
        )


if __name__ == "__main__":
    unittest.main()
