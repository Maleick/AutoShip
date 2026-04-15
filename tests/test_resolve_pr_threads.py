from __future__ import annotations

import importlib.util
import io
import json
import sys
import unittest
from pathlib import Path
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "resolve_pr_threads.py"


def load_module():
    spec = importlib.util.spec_from_file_location("resolve_pr_threads", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load resolve_pr_threads module")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def make_fake_urlopen(response_payload: dict):
    """Return a context manager that yields a file-like object with JSON payload."""
    encoded = json.dumps(response_payload).encode()

    class FakeResponse:
        def read(self):
            return encoded

        def __enter__(self):
            return self

        def __exit__(self, *args):
            pass

    return FakeResponse()


class GraphqlTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def test_graphql_sends_correct_json_body(self) -> None:
        captured_request = {}

        def fake_urlopen(req):
            captured_request["data"] = req.data
            captured_request["headers"] = dict(req.headers)
            return make_fake_urlopen({"data": {}})

        with mock.patch.object(self.module.urllib.request, "urlopen", fake_urlopen):
            self.module.graphql("mytoken", "{ viewer { login } }")

        body = json.loads(captured_request["data"])
        self.assertEqual(body["query"], "{ viewer { login } }")
        self.assertEqual(body["variables"], {})

    def test_graphql_sends_variables(self) -> None:
        captured_request = {}

        def fake_urlopen(req):
            captured_request["data"] = req.data
            return make_fake_urlopen({"data": {}})

        with mock.patch.object(self.module.urllib.request, "urlopen", fake_urlopen):
            self.module.graphql("mytoken", "query($id: ID!)", {"id": "NODE_1"})

        body = json.loads(captured_request["data"])
        self.assertEqual(body["variables"], {"id": "NODE_1"})

    def test_graphql_uses_bearer_auth_header(self) -> None:
        captured_headers = {}

        def fake_urlopen(req):
            captured_headers.update(req.headers)
            return make_fake_urlopen({"data": {}})

        with mock.patch.object(self.module.urllib.request, "urlopen", fake_urlopen):
            self.module.graphql("secret_token", "{ viewer { login } }")

        # urllib.request capitalizes header names
        auth = captured_headers.get("Authorization") or captured_headers.get("authorization")
        self.assertIsNotNone(auth)
        self.assertIn("bearer secret_token", auth)

    def test_graphql_returns_parsed_json(self) -> None:
        payload = {"data": {"viewer": {"login": "octocat"}}}

        with mock.patch.object(
            self.module.urllib.request, "urlopen",
            return_value=make_fake_urlopen(payload)
        ):
            result = self.module.graphql("tok", "{ viewer { login } }")

        self.assertEqual(result, payload)


class GetUnresolvedThreadsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def _make_threads_response(
        self, nodes: list, has_next: bool = False, end_cursor: str | None = None
    ) -> dict:
        return {
            "data": {
                "repository": {
                    "pullRequest": {
                        "reviewThreads": {
                            "pageInfo": {
                                "hasNextPage": has_next,
                                "endCursor": end_cursor,
                            },
                            "nodes": nodes,
                        }
                    }
                }
            }
        }

    def test_returns_only_unresolved_threads(self) -> None:
        nodes = [
            {
                "id": "THREAD_1",
                "isResolved": False,
                "isOutdated": False,
                "comments": {"nodes": [{"body": "fix this", "path": "src/main.rs", "author": {"login": "dev"}}]},
            },
            {
                "id": "THREAD_2",
                "isResolved": True,
                "isOutdated": False,
                "comments": {"nodes": []},
            },
        ]
        response = self._make_threads_response(nodes)

        with mock.patch.object(self.module, "graphql", return_value=response):
            threads = self.module.get_unresolved_threads("tok", 42)

        self.assertEqual(len(threads), 1)
        self.assertEqual(threads[0]["id"], "THREAD_1")

    def test_returns_empty_when_all_resolved(self) -> None:
        nodes = [
            {"id": "T1", "isResolved": True, "isOutdated": False, "comments": {"nodes": []}},
            {"id": "T2", "isResolved": True, "isOutdated": False, "comments": {"nodes": []}},
        ]
        response = self._make_threads_response(nodes)

        with mock.patch.object(self.module, "graphql", return_value=response):
            threads = self.module.get_unresolved_threads("tok", 1)

        self.assertEqual(threads, [])

    def test_returns_empty_for_no_threads(self) -> None:
        response = self._make_threads_response([])

        with mock.patch.object(self.module, "graphql", return_value=response):
            threads = self.module.get_unresolved_threads("tok", 99)

        self.assertEqual(threads, [])

    def test_exits_on_graphql_errors(self) -> None:
        error_response = {"errors": [{"message": "Not Found"}]}

        with mock.patch.object(self.module, "graphql", return_value=error_response):
            with self.assertRaises(SystemExit) as cm:
                self.module.get_unresolved_threads("tok", 1)
        self.assertEqual(cm.exception.code, 1)

    def test_paginates_across_multiple_pages(self) -> None:
        page1_node = {
            "id": "THREAD_1",
            "isResolved": False,
            "isOutdated": False,
            "comments": {"nodes": []},
        }
        page2_node = {
            "id": "THREAD_2",
            "isResolved": False,
            "isOutdated": False,
            "comments": {"nodes": []},
        }
        responses = [
            self._make_threads_response([page1_node], has_next=True, end_cursor="cursor1"),
            self._make_threads_response([page2_node], has_next=False),
        ]
        responses_iter = iter(responses)

        with mock.patch.object(self.module, "graphql", side_effect=lambda *a, **kw: next(responses_iter)):
            threads = self.module.get_unresolved_threads("tok", 5)

        self.assertEqual(len(threads), 2)
        ids = {t["id"] for t in threads}
        self.assertIn("THREAD_1", ids)
        self.assertIn("THREAD_2", ids)


class ResolveThreadTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def test_resolve_thread_returns_true_on_success(self) -> None:
        success_response = {
            "data": {
                "resolveReviewThread": {
                    "thread": {"id": "THREAD_1", "isResolved": True}
                }
            }
        }
        with mock.patch.object(self.module, "graphql", return_value=success_response):
            result = self.module.resolve_thread("tok", "THREAD_1")
        self.assertTrue(result)

    def test_resolve_thread_returns_false_on_graphql_error(self) -> None:
        error_response = {"errors": [{"message": "Thread not found"}]}
        with mock.patch.object(self.module, "graphql", return_value=error_response):
            result = self.module.resolve_thread("tok", "BAD_ID")
        self.assertFalse(result)


class MainTokenValidationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not SCRIPT_PATH.exists():
            raise unittest.SkipTest(f"Script not found: {SCRIPT_PATH}")
        cls.module = load_module()

    def test_main_exits_when_no_token(self) -> None:
        import os as stdlib_os
        args = ["resolve_pr_threads.py", "42"]
        # Simulate missing GITHUB_TOKEN and no --token argument
        with mock.patch("sys.argv", args):
            with mock.patch.dict(stdlib_os.environ, {}, clear=True):
                with self.assertRaises(SystemExit) as cm:
                    self.module.main()
                self.assertEqual(cm.exception.code, 1)

    def test_main_dry_run_prints_threads_without_resolving(self) -> None:
        unresolved = [
            {
                "id": "T1",
                "isResolved": False,
                "isOutdated": False,
                "comments": {
                    "nodes": [
                        {"body": "Please fix", "path": "src/lib.rs", "author": {"login": "reviewer"}}
                    ]
                },
            }
        ]
        args = ["resolve_pr_threads.py", "--dry-run", "--token", "tok", "123"]

        with mock.patch("sys.argv", args):
            with mock.patch.object(self.module, "get_unresolved_threads", return_value=unresolved):
                with mock.patch.object(self.module, "resolve_thread") as mock_resolve:
                    # Capture stdout so the test doesn't pollute output
                    with mock.patch("sys.stdout", new_callable=io.StringIO):
                        self.module.main()
                    # resolve_thread should NOT be called in dry-run mode
                    mock_resolve.assert_not_called()

    def test_main_resolves_threads_when_not_dry_run(self) -> None:
        unresolved = [
            {
                "id": "T1",
                "isResolved": False,
                "isOutdated": False,
                "comments": {"nodes": []},
            }
        ]
        args = ["resolve_pr_threads.py", "--token", "tok", "123"]

        with mock.patch("sys.argv", args):
            with mock.patch.object(self.module, "get_unresolved_threads", return_value=unresolved):
                with mock.patch.object(self.module, "resolve_thread", return_value=True) as mock_resolve:
                    with mock.patch("sys.stdout", new_callable=io.StringIO):
                        self.module.main()
                    mock_resolve.assert_called_once_with("tok", "T1")


if __name__ == "__main__":
    unittest.main()
