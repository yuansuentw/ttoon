"""
Compatibility entry point for the benchmark suite.

The canonical benchmark suite now lives in the repository-root `benchmarks/`
directory. This script keeps the legacy command
`cd python && uv run python tests/benchmark.py` working by delegating to the
new Python runner.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[2]
PYTHON_ROOT = REPO_ROOT / "python"
for path in (REPO_ROOT, PYTHON_ROOT):
    path_str = str(path)
    if path_str not in sys.path:
        sys.path.insert(0, path_str)

from benchmarks.python.common.config import (  # noqa: E402
    CASE_MATRIX,
    DEFAULT_ITERATIONS,
    DEFAULT_WARMUPS,
    list_cases,
)
from benchmarks.python.runner import run_benchmarks  # noqa: E402

CANONICAL_COMMAND = "uv run --project python python benchmarks/python/runner.py"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Compatibility wrapper for the benchmark suite"
    )
    parser.add_argument(
        "--dataset-root", type=Path, default=REPO_ROOT / "benchmarks/datasets/prepared"
    )
    parser.add_argument("--variant", choices=("js-basic", "extended"))
    parser.add_argument("--size", choices=("10k", "100k", "1m"))
    parser.add_argument("--shape", choices=("object", "table", "structure", "tabular"))
    parser.add_argument("--case")
    parser.add_argument("--warmups", type=int, default=DEFAULT_WARMUPS)
    parser.add_argument("--iterations", type=int, default=DEFAULT_ITERATIONS)
    parser.add_argument("--list-cases", action="store_true")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--no-auto-unpack", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    normalized_shape = _normalize_shape(args.shape)
    if args.list_cases:
        print(
            json.dumps(
                _list_case_entries(args.variant, normalized_shape),
                ensure_ascii=False,
                indent=2,
            )
        )
        return 0

    payload = run_benchmarks(
        argparse.Namespace(
            dataset_root=args.dataset_root,
            variant=args.variant,
            size=args.size,
            shape=normalized_shape,
            case=args.case,
            warmups=args.warmups,
            iterations=args.iterations,
            list_cases=False,
            no_auto_unpack=args.no_auto_unpack,
            trace_memory=False,
            benchmark_release=None,
            dataset_release=None,
        )
    )

    if args.json:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    else:
        print(render_summary(payload))

    return 0 if payload["results"] else 1


def render_summary(payload: dict[str, Any]) -> str:
    filters = payload.get("filters", {})
    filter_text = ", ".join(
        f"{key}={value}" for key, value in filters.items() if value is not None
    )
    lines = [
        "=" * 72,
        "Python benchmark compatibility wrapper",
        "=" * 72,
        f"canonical entry: {CANONICAL_COMMAND}",
        f"benchmark_release: {payload.get('benchmark_release', 'unknown')}",
        f"dataset_release: {payload.get('dataset_release', 'unknown')}",
        f"generated_at: {payload.get('generated_at', 'unknown')}",
        f"datasets: {payload.get('dataset_count', 0)}",
        f"results: {len(payload.get('results', []))}",
        f"filters: {filter_text or '(none)'}",
        "",
    ]

    results = payload.get("results", [])
    if results:
        table_rows = [
            [
                row["variant"],
                row["shape"],
                row["size"],
                row["case"],
                f"{row['stats']['mean_ms']:.3f}",
                f"{row['stats']['median_ms']:.3f}",
            ]
            for row in results
        ]
        lines.extend(
            _format_table(
                headers=["variant", "shape", "size", "case", "mean_ms", "median_ms"],
                rows=table_rows,
            )
        )
        lines.append("")
    else:
        lines.extend(
            [
                "No benchmark results to output.",
                "",
            ]
        )

    issues = payload.get("issues", [])
    if issues:
        lines.append("issues:")
        for issue in issues:
            lines.append(f"- {issue}")
    else:
        lines.append("issues: none")

    return "\n".join(lines)


def _format_table(headers: list[str], rows: list[list[str]]) -> list[str]:
    widths = [
        max(len(header), *(len(row[index]) for row in rows))
        for index, header in enumerate(headers)
    ]
    header_line = "  ".join(
        header.ljust(widths[index]) for index, header in enumerate(headers)
    )
    divider_line = "  ".join("-" * width for width in widths)
    body_lines = [
        "  ".join(cell.ljust(widths[index]) for index, cell in enumerate(row))
        for row in rows
    ]
    return [header_line, divider_line, *body_lines]


def _normalize_shape(shape: str | None) -> str | None:
    if shape == "object":
        return "structure"
    if shape == "table":
        return "tabular"
    return shape


def _list_case_entries(variant: str | None, shape: str | None) -> list[dict[str, str]]:
    entries: list[dict[str, str]] = []
    variants = (variant,) if variant else CASE_MATRIX["python"].keys()
    for variant_name in variants:
        shapes = (shape,) if shape else CASE_MATRIX["python"][variant_name].keys()
        for shape_name in shapes:
            for case_name in list_cases("python", variant_name, shape_name):
                entries.append(
                    {
                        "language": "python",
                        "variant": variant_name,
                        "shape": shape_name,
                        "case": case_name,
                    }
                )
    return entries


if __name__ == "__main__":
    raise SystemExit(main())
