# Frostreaver Cost Model

This page is the canonical documentation surface for issue [#1523](https://github.com/Maleick/TextQuest/issues/1523). The spreadsheet-compatible artifact lives at [`docs/finance/frostreaver-cost-model.csv`](../finance/frostreaver-cost-model.csv) and is intended for direct import into Excel, Google Sheets, or LibreOffice Calc.

## What The Model Covers

- Startup recovery over an 18-month break-even target.
- Monthly burn for server or hardware running costs, 36 `All Access` accounts, infrastructure, Krono replenishment, and the non-optional wife maintenance budget.
- Required monthly revenue to cover recurring burn and recover sunk costs plus startup capital inside the target window.

## Operator-Only Inputs

The repo deliberately leaves operator-only inputs blank instead of inventing accounting data that is not checked into version control. Fill these cells in the CSV before relying on the output:

- `All Access monthly price`
- `Startup Krono quantity`
- `Sunk costs to date`
- `Server/hardware running cost`
- `Infrastructure cost`
- `Monthly Krono replenishment budget`
- `Current monthly revenue`
- `Wife maintenance budget`
- `Working capital reserve`
- `Risk tolerance profile`

The `Model completeness flag` row returns `needs operator inputs` until every required operator input in `C4:C15` is populated. The `Risk tolerance profile` row stays outside that completeness range because it guides reserve sizing rather than feeding the monthly burn or break-even formulas directly.

## Formula Notes

The model keeps the core math in CSV formulas so the sheet stays transparent after import:

- `Monthly subscription burn` = managed accounts multiplied by the `All Access` unit price.
- `Startup Krono capital` = startup Krono quantity multiplied by the Krono unit price assumption.
- `Monthly burn rate` = recurring operating costs plus wife maintenance budget.
- `Required monthly revenue for 18-month break-even` = monthly burn plus one-time startup recovery spread across the 18-month break-even target.
- `Break-even months at current revenue` returns `UNBOUNDED` when current revenue does not clear recurring burn.

## Default Assumptions Already Seeded

- `Target break-even window (months)` is set to `18` from the issue assumption.
- `Managed accounts` is set to `36`.
- `Krono unit price` is seeded at `17.99` USD per Krono as a visible placeholder assumption.

## Risk-Tolerance Interpretation

`Risk tolerance profile` is documented even though it is not part of the formula range yet. Use it to decide how much `Working capital reserve` to carry:

- `aggressive early push`: higher startup reserve, higher monetization expectation, faster recovery target pressure.
- `steady grind`: smaller reserve, lower monthly monetization pressure, slower scaling assumptions.

This keeps the cost accounting sheet aligned with the issue requirement without pretending the operator decision has already been made.

---

## See Also

- **[Frostreaver Farming & XP Guide](Frostreaver-Farming-Guide.md)** — Complete leveling zone progression, plat farming locations, raid targets, encounter locking, and randomized loot meta
- **[Frostreaver Starting City Logistics](Frostreaver-Starting-City-Logistics.md)** — Detailed 36-account matrix, city split strategy, early leveling routes, and gear handoff logistics for launch day
