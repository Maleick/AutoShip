---
title: Graph Analysis Chart Exports
---

# Graph Analysis Chart Exports

Use this page to import the operator/combat graph exports that were generated for charting and review.

## Bundle

- `artifacts/operator_chart_bundle.zip`
- `artifacts/operator_chart_bundle_manifest.json`
- `scripts/render_graph_chart_bundle.py`

## Files Included

- `operator_chart_summary_1hop.json`
- `operator_1hop_summary.csv`
- `operator_flow_1hop.csv`
- `operator_2hop_summary.json`
- `operator_2hop_summary.csv`
- `operator_flow_2hop.csv`
- `components_vs_combat_handle_crossover.json`
- `components_vs_combat_handle_crossover.csv`
- `operator_2hop_combat_flow_list.csv`
- `operator_2hop_combat_flow_list.json`
- `graph_metrics_commit_trend.csv`
- `graph_metrics_commit_trend.json`
- `operator_chart_summary_matrix.csv`
- `operator_chart_template_spec.json`
- `operator_top5_combat_flows_vega_spec.json`

## Quick Import

1. Unzip `artifacts/operator_chart_bundle.zip`.
2. Read `operator_chart_bundle_manifest.json`.
3. Import CSV files into chart tooling by `metric,value`/`flow,criticality,...` columns.
4. Use JSON files for drill-down cards and dashboard annotations.
5. For single-sheet chart import, use `operator_chart_summary_matrix.csv`.
6. Render quickly in terminal:
   - `python3 scripts/render_graph_chart_bundle.py --template artifacts/operator_chart_template_spec.json`
7. For top-5 combat flows chart, use:
   - `artifacts/operator_top5_combat_flows_vega_spec.json` in a Vega-Lite viewer.

## Notes

- Operator-facing runtime flows are not detected for `App.tsx` + `OperatorDashboard.tsx` as direct top-level affected flows in the current 1-hop slice.
- Combat hotspot flows are concentrated in `textquest-dll/src/combat/state.rs` at 2-hop scope.
- Hub/surprising-connection tools (`get_hub_nodes`, `get_surprising_connections`) still error in this session with `str object has no attribute resolve`.

## Consolidated export

`operator_chart_summary_matrix.csv` provides one wide-format table combining:
- operator KPIs (1-hop and 2-hop),
- commit trend snapshots and deltas,
- top combat flow indicators,
- cross-module flow matrix signals.
