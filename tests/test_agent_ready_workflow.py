from __future__ import annotations

from pathlib import Path
import re
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = REPO_ROOT / ".github" / "workflows" / "automation.yml"


class AutomationWorkflowTests(unittest.TestCase):
    def test_closed_issues_trigger_cleanup(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        # In automation.yml it's under issues: types:
        self.assertRegex(
            text,
            re.compile(
                r"types:\s*\[\s*opened,\s*edited,\s*reopened,\s*closed,\s*labeled,\s*unlabeled\s*\]"
            ),
        )

    def test_pr_automation_uses_only_pull_request_target(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        self.assertNotIn("\n  pull_request:\n", text)
        self.assertRegex(
            text,
            re.compile(
                r"pull_request_target:\s*\n\s*types:\s*\[\s*labeled,\s*reopened,\s*synchronize,\s*ready_for_review,\s*closed\s*\]"
            ),
        )

    def test_roadmap_container_regex_keeps_js_tokens(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn(r'/^M(\d+|x)\b/i.test((title || "").trim());', text)

    def test_linked_pr_detection_uses_rest_timeline_api(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("GET /repos/{owner}/{repo}/issues/{issue_number}/timeline", text)
        self.assertNotIn("timelineItems(first: 100", text)

    def test_post_merge_sync_deletes_branch_inline(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("contents: write", text)
        self.assertIn("github.rest.git.deleteRef", text)

    def test_ready_label_is_never_added_to_closed_issues(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
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
