from __future__ import annotations

import re
from pathlib import Path
import unittest

REPO_ROOT = Path(__file__).resolve().parents[1]
ISSUE_FILES = [
    REPO_ROOT / "docs" / "ISSUE_INDEX.md",
    REPO_ROOT / "docs" / "FINAL_ISSUE_INDEX.md",
]
ROW_RE = re.compile(r"^\|\s*#(\d+[a-z]?)\s*\|\s*([^|]+?)\s*\|")


class IssueIndexConsistencyTests(unittest.TestCase):
    def test_each_issue_id_has_single_title_within_each_index(self) -> None:
        for path in ISSUE_FILES:
            titles_by_issue: dict[str, set[str]] = {}
            for line in path.read_text(encoding="utf-8").splitlines():
                match = ROW_RE.match(line)
                if not match:
                    continue
                issue_id, title = match.groups()
                normalized = " ".join(title.split())
                titles_by_issue.setdefault(issue_id, set()).add(normalized)

            conflicting = {
                issue_id: sorted(titles)
                for issue_id, titles in titles_by_issue.items()
                if len(titles) > 1
            }

            self.assertEqual(
                {},
                conflicting,
                msg=(
                    f"{path.name} reuses issue IDs for multiple titles. "
                    f"Conflicts: {conflicting}"
                ),
            )


if __name__ == "__main__":
    unittest.main()
