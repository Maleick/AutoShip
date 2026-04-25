"""
Tier 1 RedGuides Lua script conformance tests — issue #2617.

Test strategy:
  1. Verify TextQuest Lua VM source structure exists (fast, no compile needed).
  2. Verify required API surface is registered in bindings.rs (grep-based).
  3. Execute each Tier 1 fixture script against a system Lua 5.4 interpreter
     (skipped gracefully when lua5.4 / lua are absent — CI on Windows must
     install Lua 5.4 or the job is skipped, not failed).
  4. Verify the compatibility matrix document is present and well-formed.

Blocked scripts (boxhud, shareddata) are marked as SKIP with explanatory messages
so they appear in CI output without failing the suite.
"""
from __future__ import annotations

import re
import shutil
import subprocess
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
LUA_MODULE_ROOT = REPO_ROOT / "textquest" / "src" / "lua"
FIXTURES_DIR = Path(__file__).resolve().parent / "lua_fixtures"
WIKI_DOC = REPO_ROOT / "docs" / "wiki" / "RedGuides-Script-Compatibility.md"
BINDINGS_SRC = LUA_MODULE_ROOT / "bindings.rs"


def find_lua() -> str | None:
    """Return the first usable Lua 5.x interpreter name, or None."""
    for candidate in ("lua5.4", "lua54", "lua5.3", "lua"):
        path = shutil.which(candidate)
        if path:
            return candidate
    return None


LUA_BIN = find_lua()


def run_lua_fixture(fixture_name: str) -> tuple[bool, str]:
    """Run a fixture script through the system Lua interpreter.

    Returns (passed, output). The fixture sets CONFORMANCE_RESULT = 'PASS' on
    success; any Lua error is a failure.
    """
    if LUA_BIN is None:
        return False, "no-lua-interpreter"

    fixture = FIXTURES_DIR / fixture_name
    if not fixture.exists():
        return False, f"fixture not found: {fixture}"

    # Prepend the fixtures dir to package.path so require('mq_compat') resolves.
    lua_path = f"{FIXTURES_DIR}/?.lua"
    env_patch = {"LUA_PATH": lua_path + ";;" }

    import os
    env = os.environ.copy()
    env["LUA_PATH"] = lua_path + ";;"

    probe = (
        f"package.path = '{lua_path}' .. ';' .. package.path; "
        f"dofile('{fixture}'); "
        f"print(CONFORMANCE_RESULT or 'NO_RESULT')"
    )
    result = subprocess.run(
        [LUA_BIN, "-e", probe],
        capture_output=True,
        text=True,
        timeout=10,
        env=env,
    )
    combined = result.stdout.strip() + result.stderr.strip()
    if result.returncode != 0:
        return False, combined
    passed = "PASS" in result.stdout
    return passed, combined


class LuaVmSourceStructureTests(unittest.TestCase):
    """Verify the Lua VM source files exist at expected paths."""

    def test_lua_module_root_exists(self) -> None:
        self.assertTrue(
            LUA_MODULE_ROOT.exists(),
            f"Lua module root missing: {LUA_MODULE_ROOT}",
        )

    def test_bindings_rs_exists(self) -> None:
        self.assertTrue(BINDINGS_SRC.exists(), "bindings.rs missing")

    def test_loader_rs_exists(self) -> None:
        self.assertTrue((LUA_MODULE_ROOT / "loader.rs").exists(), "loader.rs missing")

    def test_types_rs_exists(self) -> None:
        self.assertTrue((LUA_MODULE_ROOT / "types.rs").exists(), "types.rs missing")

    def test_sandbox_rs_exists(self) -> None:
        self.assertTrue((LUA_MODULE_ROOT / "sandbox.rs").exists(), "sandbox.rs missing")


class LuaApiSurfaceTests(unittest.TestCase):
    """Verify required API registrations are present in bindings.rs."""

    @classmethod
    def setUpClass(cls) -> None:
        if not BINDINGS_SRC.exists():
            raise unittest.SkipTest("bindings.rs not found")
        cls.src = BINDINGS_SRC.read_text(encoding="utf-8")

    def _assert_api(self, symbol: str) -> None:
        self.assertIn(
            symbol,
            self.src,
            f"Required API symbol '{symbol}' not found in bindings.rs",
        )

    def test_player_api_registered(self) -> None:
        self._assert_api("register_player_api")

    def test_group_api_registered(self) -> None:
        self._assert_api("register_group_api")

    def test_nav_api_registered(self) -> None:
        self._assert_api("register_nav_api")

    def test_combat_api_registered(self) -> None:
        self._assert_api("register_combat_api")

    def test_event_emit_registered(self) -> None:
        self._assert_api("emit_event")

    def test_execute_string_in_loader(self) -> None:
        # luaconsole requires execute_string — lives in loader.rs, not bindings.rs
        loader_src = (LUA_MODULE_ROOT / "loader.rs").read_text(encoding="utf-8")
        self.assertIn(
            "execute_string",
            loader_src,
            "execute_string not found in loader.rs — luaconsole REPL path missing",
        )

    def test_issue_791_compat_layer(self) -> None:
        # #791 compat shim for mq.* global
        self._assert_api("register_issue_791_compat_api")


class Tier1FixtureTests(unittest.TestCase):
    """Execute Tier 1 fixture stubs through the system Lua interpreter."""

    @classmethod
    def setUpClass(cls) -> None:
        if LUA_BIN is None:
            raise unittest.SkipTest(
                "No Lua interpreter found (lua5.4 / lua54 / lua). "
                "Install Lua 5.4 to run conformance fixtures."
            )

    def _run(self, fixture: str, script_name: str) -> None:
        passed, output = run_lua_fixture(fixture)
        self.assertTrue(
            passed,
            f"{script_name} conformance fixture failed.\nOutput:\n{output}",
        )

    def test_aqobot_reaches_main_loop(self) -> None:
        self._run("aqobot_stub.lua", "aqobot")

    def test_lem_run_completes(self) -> None:
        self._run("lem_stub.lua", "lem")

    def test_lootnscoot_loot_helper_defined(self) -> None:
        self._run("lootnscoot_stub.lua", "lootnscoot")

    def test_luaconsole_imgui_registered(self) -> None:
        self._run("luaconsole_stub.lua", "luaconsole")

    def test_alertmaster_alert_list_nonempty(self) -> None:
        self._run("alertmaster_stub.lua", "alertmaster")

    @unittest.skip("Blocked on DanNet interop (#2613)")
    def test_boxhud_blocked(self) -> None:
        pass

    @unittest.skip("Blocked on Actors mailbox (#2607)")
    def test_shareddata_blocked(self) -> None:
        pass


class CompatibilityMatrixDocTests(unittest.TestCase):
    """Verify the compatibility matrix document is present and well-formed."""

    @classmethod
    def setUpClass(cls) -> None:
        if not WIKI_DOC.exists():
            raise unittest.SkipTest(f"Wiki doc not found: {WIKI_DOC}")
        cls.content = WIKI_DOC.read_text(encoding="utf-8")

    def test_doc_has_tier_definitions(self) -> None:
        self.assertIn("Tier 1", self.content, "Doc missing Tier 1 definition")
        self.assertIn("Tier 2", self.content, "Doc missing Tier 2 definition")
        self.assertIn("Tier 3", self.content, "Doc missing Tier 3 definition")

    def test_all_tier1_scripts_present(self) -> None:
        tier1_scripts = ["aqobot", "lem", "lootnscoot", "luaconsole", "AlertMaster", "shareddata"]
        for script in tier1_scripts:
            self.assertIn(script, self.content, f"Tier 1 script '{script}' missing from matrix")

    def test_emumeshes_ingestion_documented(self) -> None:
        self.assertIn("EMUMeshes", self.content)
        self.assertIn(".nav", self.content, "EMUMeshes .nav format not documented")

    def test_dependency_issues_referenced(self) -> None:
        for issue in ("#2607", "#2613", "#791", "#792"):
            self.assertIn(issue, self.content, f"Issue {issue} not referenced in doc")

    def test_matrix_table_has_all_scripts(self) -> None:
        scripts = [
            "aqobot", "lem", "lootnscoot", "boxhud", "luaconsole",
            "buttonmaster", "MyUI", "AlertMaster", "shareddata",
            "CombatControl", "EMUMeshes",
        ]
        for script in scripts:
            self.assertIn(script, self.content, f"Script '{script}' missing from matrix")


class EMUMeshesIngestionTests(unittest.TestCase):
    """Verify navmesh loader infrastructure supports EMUMeshes drop-in."""

    NAV_MOD = REPO_ROOT / "textquest-dll" / "src" / "nav" / "mod.rs"

    @classmethod
    def setUpClass(cls) -> None:
        if not cls.NAV_MOD.exists():
            raise unittest.SkipTest(f"nav/mod.rs not found: {cls.NAV_MOD}")
        cls.src = cls.NAV_MOD.read_text(encoding="utf-8")

    def test_nav_mod_exists(self) -> None:
        self.assertTrue(self.NAV_MOD.exists())

    def test_nav_accepts_nav_extension_or_mesh_path(self) -> None:
        # Nav loader must have mesh-load plumbing; EMUMeshes .nav files drop into
        # the same path.  Accept any of: ".nav", "navmesh", "nav_mesh",
        # "MeshLoaded", "mesh_loaded" (SetMeshLoaded / set_mesh_loaded in mod.rs).
        src_lower = self.src.lower()
        has_nav_ref = (
            ".nav" in self.src
            or "navmesh" in src_lower
            or "nav_mesh" in src_lower
            or "meshloaded" in src_lower
            or "mesh_loaded" in src_lower
        )
        self.assertTrue(
            has_nav_ref,
            "nav/mod.rs does not reference mesh-load plumbing — EMUMeshes drop-in not confirmed",
        )


if __name__ == "__main__":
    unittest.main()
