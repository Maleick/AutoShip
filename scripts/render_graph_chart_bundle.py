#!/usr/bin/env python3

import argparse
import csv
import json
from pathlib import Path


def read_json(path):
    with Path(path).open("r", encoding="utf-8") as handle:
        return json.load(handle)


def read_csv(path):
    with Path(path).open("r", encoding="utf-8", newline="") as handle:
        return list(csv.DictReader(handle))


def to_number(value):
    if value is None:
        return None

    text = str(value).strip()
    if not text:
        return None

    try:
        return float(text)
    except ValueError:
        return None


def render_bar(value, max_value, width=24):
    if max_value <= 0:
        return "." * width

    ratio = value / max_value if value is not None else 0
    if ratio < 0:
        ratio = 0
    if ratio > 1:
        ratio = 1

    fill = int(round(ratio * width))
    return "#" * fill + "." * (width - fill)


def format_value(value):
    if value is None:
        return "-"

    if isinstance(value, float) and value.is_integer():
        return f"{int(value)}"

    if isinstance(value, float):
        return f"{value:.4f}"

    return str(value)


def apply_filter(rows, chart):
    section = chart.get("filter", {}).get("section")
    if not section:
        return rows

    return [row for row in rows if row.get("section") == section]


def render_line_chart(chart, rows):
    title = chart.get("title", "line chart")
    x_field = chart["x"]
    series = chart.get("series", [])

    print(f"\n## {title}")
    print(f"source={chart['source']}")

    for item in series:
        field = item["field"]
        label = item.get("label", field)
        values = []

        for row in rows:
            x_value = row.get(x_field, "")
            y_value = to_number(row.get(field))
            values.append((x_value, y_value))

        max_value = max((v for _, v in values if v is not None), default=0)

        print(f"\n{label}")
        for x_value, y_value in values:
            bar = render_bar(y_value if y_value is not None else 0, max_value)
            print(f"  {str(x_value):<14} {format_value(y_value):>10} {bar}")


def render_bar_chart(chart, rows):
    title = chart.get("title", "bar chart")
    x_field = chart["x"]
    series = chart.get("series", [])
    sort = chart.get("sort")
    limit = 0

    if sort:
        sort_field = sort.get("field")
        reverse = sort.get("order", "desc").lower() != "asc"
        rows = sorted(rows, key=lambda row: to_number(row.get(sort_field)) or 0, reverse=reverse)
        limit = sort.get("limit", 0)

    if limit and isinstance(limit, int):
        rows = rows[:limit]

    all_values = []
    for row in rows:
        for item in series:
            all_values.append(to_number(row.get(item["field"])) or 0)

    max_value = max(all_values, default=0)

    print(f"\n## {title}")
    print(f"source={chart['source']}")

    for row in rows:
        name = row.get(x_field, "")
        print(f"\n{name}")
        for item in series:
            label = item.get("label", item["field"])
            value = to_number(row.get(item["field"]))
            printable = format_value(value)
            bar = render_bar(value if value is not None else 0, max_value)
            print(f"  {label:<16} {printable:>10} {bar}")


def render_matrix_chart(chart, rows):
    title = chart.get("title", "matrix")
    row_field = chart["rows"]
    columns = chart.get("columns", [])
    value_field = chart.get("value", "")

    if isinstance(row_field, str):
        row_field = [row_field]

    print(f"\n## {title}")
    print(f"source={chart['source']}")

    col_width = max(len(str(c)) for c in columns) if columns else 1
    row_label_width = max(len(str(row.get(row_field[0], ""))) for row in rows) if rows else 0
    row_label_width = max(row_label_width, len("metric"))

    header = " " * (row_label_width + 2)
    for column in columns:
        header += f"{str(column):>{col_width + 2}}"
    print(header)
    print(" " * (row_label_width + 2) + "-" * (len(header) - row_label_width - 2))

    for row in rows:
        key = row.get(row_field[0], "")
        line = f"{str(key):<{row_label_width + 2}}"

        for column in columns:
            value = row.get(column, row.get(value_field, ""))
            line += f"{str(value):>{col_width + 2}}"

        if value_field:
            context = row.get("context")
            if context:
                line += f"  # {context}"

        print(line)


def render_chart(chart, rows):
    chart_type = chart.get("type", "bar").lower()

    if chart_type == "line":
        render_line_chart(chart, rows)
    elif chart_type == "bar":
        render_bar_chart(chart, rows)
    elif chart_type == "matrix":
        render_matrix_chart(chart, rows)
    else:
        print(f"Unsupported chart type: {chart_type}")


def render(template, chart_ids=None):
    charts = template.get("charts", [])
    selected = set(chart_ids or [])

    for chart in charts:
        chart_id = chart.get("id")
        if selected and chart_id not in selected:
            continue

        source = chart.get("source")
        rows = read_csv(source)
        rows = apply_filter(rows, chart)
        render_chart(chart, rows)

    if selected and not any(chart.get("id") in selected for chart in charts):
        raise ValueError(f"No matching chart ids: {', '.join(sorted(selected))}")


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description="Render operator chart template output in terminal")
    parser.add_argument("--template", default="artifacts/operator_chart_template_spec.json", help="Path to chart template JSON")
    parser.add_argument("--chart", action="append", help="Render only this chart id (can be repeated)")
    return parser.parse_args(argv)


def main():
    args = parse_args()
    template = read_json(args.template)

    print(f"Template: {args.template}")
    render(template, chart_ids=args.chart)


if __name__ == "__main__":
    main()
