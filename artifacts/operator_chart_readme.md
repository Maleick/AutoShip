# Operator Graph Chart Exports

This folder contains import-ready exports used for charts and PR reporting.

## Primary operator slice
- `operator_chart_summary_1hop.json` and `operator_1hop_summary.csv`: 1-hop impact from `App.tsx` + `OperatorDashboard.tsx`.
- `operator_2hop_summary.json` and `operator_2hop_summary.csv`: 1-hop and 2-hop impact expansion.
- `operator_flow_1hop.csv` / `operator_flow_2hop.csv`: flow tables for each scope (note: runtime flow list is empty for App/Operator in these slices).

## Cross-module view
- `components_vs_combat_handle_crossover.json` and `.csv`: high-level UI (`components-handle`) vs combat (`combat-target`) profile and coupling signal.

## Combat flow deep list
- `operator_2hop_combat_flow_list.csv` and `.json`: top 2-hop combat flows (76 total impacted in combat file scope), including criticality, node size, and source references.

## Trend
- `graph_metrics_commit_trend.csv` and `.json`: 3-commit trend of graph metrics (nodes/edges/files and top community sizes).

## Consolidated matrix
- `operator_chart_summary_matrix.csv`: single wide-format sheet combining operator KPIs, 2-hop expansion, commit trend metrics, and top flow / coupling summaries.

## Bundle
- `operator_chart_bundle.zip`: all files above plus manifest for tooling automation.
- `operator_chart_bundle_manifest.json`: machine-readable index of bundle contents.

## Recommended chart import
1. Load the manifest first.
2. Import CSV files for time-series and KPI style charts.
3. Use JSON files as annotations for dashboard narrative cards.
4. For one-stop dashboard import, use `operator_chart_summary_matrix.csv`.
5. Render these exports in-terminal using:
   - `python3 scripts/render_graph_chart_bundle.py --template artifacts/operator_chart_template_spec.json`

## Chart spec templates
- `artifacts/operator_chart_template_spec.json`: chart definitions for terminal or script-based rendering.
- `artifacts/operator_top5_combat_flows_vega_spec.json`: polished top-5 combat flow chart spec for Vega-Lite viewers.
