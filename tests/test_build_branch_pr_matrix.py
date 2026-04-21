from __future__ import annotations

import json
import unittest
from unittest import mock

from scripts import build_branch_pr_matrix


class BuildBranchPrMatrixTests(unittest.TestCase):
    def test_open_prs_skips_invalid_json_output(self) -> None:
        with mock.patch.object(
            build_branch_pr_matrix,
            "run",
            return_value="gh: authentication failed",
        ):
            self.assertEqual(build_branch_pr_matrix.open_prs(), {})

    def test_open_prs_only_keeps_same_owner_head_refs(self) -> None:
        payload = [
            {
                "number": 10,
                "title": "same owner",
                "url": "https://example.com/pr/10",
                "headRefName": "feature/test",
                "headRepositoryOwner": {"login": "Maleick"},
            },
            {
                "number": 11,
                "title": "fork copy",
                "url": "https://example.com/pr/11",
                "headRefName": "feature/test",
                "headRepositoryOwner": {"login": "someone-else"},
            },
            {
                "number": 12,
                "title": "missing ref",
                "url": "https://example.com/pr/12",
                "headRefName": "",
                "headRepositoryOwner": {"login": "Maleick"},
            },
        ]

        with mock.patch.object(
            build_branch_pr_matrix,
            "run",
            return_value=json.dumps(payload),
        ):
            prs = build_branch_pr_matrix.open_prs()

        self.assertEqual(list(prs), ["feature/test"])
        self.assertEqual(prs["feature/test"]["number"], 10)

    def test_prefers_non_origin_upstream_when_origin_branch_is_missing(self) -> None:
        with mock.patch.object(
            build_branch_pr_matrix,
            "parse_local_branches",
            return_value={"feature/test": "upstream/feature/test"},
        ), mock.patch.object(
            build_branch_pr_matrix,
            "parse_remote_branches",
            return_value={"upstream/feature/test"},
        ), mock.patch.object(
            build_branch_pr_matrix,
            "open_prs",
            return_value={},
        ), mock.patch.object(
            build_branch_pr_matrix,
            "is_clean",
            return_value=True,
        ), mock.patch.object(
            build_branch_pr_matrix,
            "upstream_ahead_behind",
            return_value=(0, 0),
        ) as ahead_behind:
            rows = build_branch_pr_matrix.render_matrix_rows()

        self.assertEqual(
            rows,
            [("feature/test", "upstream/feature/test", "none", "no-PR", "local")],
        )
        ahead_behind.assert_called_once_with("upstream/feature/test")

    def test_render_matrix_uses_markdown_links_and_local_context(self) -> None:
        with mock.patch.object(
            build_branch_pr_matrix,
            "parse_local_branches",
            return_value={"feature/test": "origin/feature/test"},
        ), mock.patch.object(
            build_branch_pr_matrix,
            "parse_remote_branches",
            return_value={"origin/feature/test"},
        ), mock.patch.object(
            build_branch_pr_matrix,
            "open_prs",
            return_value={
                "feature/test": {
                    "number": 42,
                    "title": "example",
                    "url": "https://example.com/pr/42",
                }
            },
        ), mock.patch.object(
            build_branch_pr_matrix,
            "is_clean",
            return_value=True,
        ), mock.patch.object(
            build_branch_pr_matrix,
            "upstream_ahead_behind",
            return_value=(1, 0),
        ):
            rows = build_branch_pr_matrix.render_matrix_rows()

        self.assertEqual(
            rows,
            [("feature/test", "origin/feature/test", "[#42](https://example.com/pr/42)", "ahead", "local")],
        )


if __name__ == "__main__":
    unittest.main()
