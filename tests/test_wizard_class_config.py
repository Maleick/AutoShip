from __future__ import annotations

from pathlib import Path
import tomllib
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]


class WizardClassConfigTests(unittest.TestCase):
    def test_wizard_level_62_override_covers_levels_62_through_64(self) -> None:
        config = tomllib.loads(
            (REPO_ROOT / "config" / "classes" / "wizard.toml").read_text(
                encoding="utf-8"
            )
        )

        overrides = config["level_overrides"]
        level_62_override = next(
            override
            for override in overrides
            if override["min_level"] == 62
        )

        self.assertEqual(level_62_override["max_level"], 64)
        self.assertEqual(
            [ability["name"] for ability in level_62_override["combat_abilities"]],
            [
                "White Fire",
                "Ancient: Destruction of Ice",
                "Lure of Thunder",
                "Jyll's Wave of Heat",
                "Harvest of Druzzil",
            ],
        )

    def test_wizard_level_65_override_starts_after_the_62_to_64_window(self) -> None:
        config = tomllib.loads(
            (REPO_ROOT / "config" / "classes" / "wizard.toml").read_text(
                encoding="utf-8"
            )
        )

        overrides = config["level_overrides"]
        level_65_override = next(
            override
            for override in overrides
            if override["min_level"] == 65
        )

        self.assertEqual(level_65_override["max_level"], 65)
        self.assertEqual(
            [ability["name"] for ability in level_65_override["combat_abilities"]],
            [
                "Fire of Tallon",
                "Draught of E`ci",
                "Lure of Thunder",
                "Jyll's Wave of Heat",
                "Harvest of Druzzil",
            ],
        )
