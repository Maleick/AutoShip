from __future__ import annotations

from pathlib import Path
import re
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = REPO_ROOT / ".github" / "workflows" / "agent-ready.yml"


class AgentReadyWorkflowTests(unittest.TestCase):
    def test_closed_issues_trigger_cleanup(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        self.assertRegex(
            text,
            re.compile(
                r"types:\s*\[\s*opened,\s*edited,\s*reopened,\s*closed,\s*labeled,\s*unlabeled\s*\]"
            ),
        )

    def test_roadmap_container_regex_keeps_js_tokens(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn(r'return /^M(\d+|x)\b/i.test((title || "").trim());', text)

    def test_ready_label_is_never_added_to_closed_issues(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        self.assertRegex(
            text,
            re.compile(
                r'async function ensureReadyState\(\{\s*number,\s*state = "open",\s*title = "",\s*labels: currentLabels = \[\]\s*\}\)'
            ),
        )
        self.assertIn('const isClosed = state !== "open";', text)
        self.assertRegex(
            text,
            re.compile(r"if \(\(isClosed \|\| shouldSkip \|\| shouldWithholdReady\) && hasReady\) \{"),
        )
        self.assertRegex(
            text,
            re.compile(
                r"if \(!isClosed && !shouldSkip && !shouldWithholdReady && !hasReady\) \{"
            ),
        )


if __name__ == "__main__":
    unittest.main()
