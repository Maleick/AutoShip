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
                r"pull_request_target:\s*\n\s*types:\s*\[\s*labeled,\s*unlabeled,\s*reopened,\s*synchronize,\s*ready_for_review,\s*converted_to_draft,\s*closed\s*\]"
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

    def test_ready_label_requires_structured_issue_body(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")

        self.assertIn("const requiredReadySectionGroups = [", text)
        for section in (
            "Task Description",
            "Scope",
            "Implementation Notes",
            "Testing",
            "Acceptance Criteria",
            "Done when (Acceptance Criteria)",
            "Acceptance",
        ):
            with self.subTest(section=section):
                self.assertIn(section, text)
        self.assertIn("#{2,3}", text)
        self.assertIn("hasRequiredReadySections(issue.body || \"\")", text)
        self.assertIn("missingRequiredSections", text)

    def test_automation_has_no_dead_schedule_conditions(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")

        self.assertNotIn("github.event_name == 'schedule'", text)

    def test_auto_merge_uses_squash_and_blocking_labels(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")

        self.assertIn("mergeMethod: SQUASH", text)
        self.assertIn("extractLinkedIssueNumbers", text)
        self.assertIn("linkedIssueHasBlockingLabels", text)
        self.assertIn("disablePullRequestAutoMerge", text)
        self.assertIn("autoMergeBlockReason", text)
        for label in ("merge:block", "agent:blocked", "human:required", "risk:high"):
            with self.subTest(label=label):
                self.assertIn(label, text)
        self.assertIn("pr.data.draft", text)


if __name__ == "__main__":
    unittest.main()
