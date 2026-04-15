from __future__ import annotations

import csv
from pathlib import Path
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
CSV_PATH = REPO_ROOT / "docs" / "finance" / "frostreaver-cost-model.csv"
WIKI_PATH = REPO_ROOT / "docs" / "wiki" / "Frostreaver-Cost-Model.md"
HOME_PATH = REPO_ROOT / "docs" / "wiki" / "Home.md"
SIDEBAR_PATH = REPO_ROOT / "docs" / "wiki" / "_Sidebar.md"


class FrostreaverCostModelTests(unittest.TestCase):
    maxDiff = None

    def load_rows(self) -> list[dict[str, str]]:
        self.assertTrue(CSV_PATH.exists(), f"missing cost model artifact: {CSV_PATH}")
        with CSV_PATH.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            return list(reader)

    def rows_by_item(self) -> dict[str, dict[str, str]]:
        return {row["Line Item"]: row for row in self.load_rows()}

    def test_cost_model_csv_has_expected_structure(self) -> None:
        rows = self.load_rows()
        self.assertEqual(
            rows[0].keys(),
            {"Section", "Line Item", "Value", "Unit", "Source / Notes"},
        )

    def test_cost_model_csv_captures_required_inputs_and_formulas(self) -> None:
        rows = self.rows_by_item()

        self.assertEqual(rows["Target break-even window (months)"]["Value"], "18")
        self.assertEqual(rows["Managed accounts"]["Value"], "36")
        self.assertEqual(rows["Krono unit price"]["Value"], "17.99")

        expected_formulas = {
            "Monthly subscription burn": "=C3*C4",
            "Startup Krono capital": "=C5*C6",
            "Monthly burn rate": "=SUM(C9:C12)+C14",
            "Required monthly revenue for 18-month break-even": "=C16+((SUM(C7:C8)+C15)/C2)",
            "Monthly net after burn": "=C13-C16",
            "Break-even months at current revenue": '=IF(C18<=0,"UNBOUNDED",ROUNDUP((SUM(C7:C8)+C15)/C18,0))',
            "Revenue shortfall vs 18-month target": "=MAX(0,C17-C13)",
            "Model completeness flag": '=IF(COUNTBLANK(C4:C15)=0,"complete","needs operator inputs")',
        }
        for line_item, formula in expected_formulas.items():
            with self.subTest(line_item=line_item):
                self.assertEqual(rows[line_item]["Value"], formula)

    def test_wiki_page_exposes_model_and_missing_inputs(self) -> None:
        self.assertTrue(WIKI_PATH.exists(), f"missing wiki page: {WIKI_PATH}")
        text = WIKI_PATH.read_text(encoding="utf-8")
        self.assertIn("docs/finance/frostreaver-cost-model.csv", text)
        self.assertIn("operator-only inputs", text)
        self.assertIn("18-month break-even target", text)
        self.assertIn("Krono", text)
        self.assertIn("All Access", text)

    def test_home_and_sidebar_link_to_cost_model_page(self) -> None:
        home = HOME_PATH.read_text(encoding="utf-8")
        sidebar = SIDEBAR_PATH.read_text(encoding="utf-8")
        self.assertIn("[Frostreaver Cost Model](Frostreaver-Cost-Model.md)", home)
        self.assertIn("[Frostreaver Cost Model](Frostreaver-Cost-Model)", sidebar)


if __name__ == "__main__":
    unittest.main()
