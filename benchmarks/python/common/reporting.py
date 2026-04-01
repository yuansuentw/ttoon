from __future__ import annotations

import csv
import json
import platform
import subprocess
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from benchmarks.python.common.config import choose_baseline, case_direction
from benchmarks.python.common.manifests import (
    ReleaseMetadata,
    validate_raw_payload_release,
)


def utc_now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def collect_host_environment() -> dict[str, Any]:
    return {
        "os": platform.system(),
        "os_release": platform.release(),
        "platform": platform.platform(),
        "architecture": platform.machine(),
        "cpu_model": detect_cpu_model(),
        "python_version": platform.python_version(),
        "node_version": _read_command_output(["node", "--version"]),
        "rust_version": _read_command_output(["rustc", "--version"]),
    }


def detect_cpu_model() -> str:
    system = platform.system()
    if system == "Darwin":
        value = _read_command_output(["sysctl", "-n", "machdep.cpu.brand_string"])
        if value:
            return value
    if system == "Linux":
        cpuinfo = Path("/proc/cpuinfo")
        if cpuinfo.exists():
            for line in cpuinfo.read_text(
                encoding="utf-8", errors="ignore"
            ).splitlines():
                if line.startswith("model name"):
                    return line.split(":", 1)[1].strip()
    return platform.processor() or "unknown"


def write_json(path: Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def flatten_results(raw_payloads: list[dict[str, Any]]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for payload in raw_payloads:
        language = payload.get("language", "unknown")
        environment = payload.get("environment", {})
        for result in payload.get("results", []):
            row = dict(result)
            row["language"] = language
            row["benchmark_release"] = payload.get("benchmark_release")
            row["dataset_release"] = payload.get("dataset_release")
            row["cpu_model"] = environment.get("cpu_model")
            row["python_version"] = environment.get("python_version")
            row["node_version"] = environment.get("node_version")
            row["rust_version"] = environment.get("rust_version")
            rows.append(row)
    return rows


def build_summary(
    raw_payloads: list[dict[str, Any]],
    release_metadata: ReleaseMetadata,
) -> dict[str, Any]:
    for payload in raw_payloads:
        validate_raw_payload_release(
            payload,
            release_metadata,
            source_label=f"language={payload.get('language', 'unknown')}",
        )

    rows = flatten_results(raw_payloads)
    grouped: dict[tuple[str, str, str, str, str], list[dict[str, Any]]] = defaultdict(
        list
    )
    for row in rows:
        direction = case_direction(row["case"])
        key = (
            row["language"],
            row["variant"],
            row["shape"],
            row["size"],
            direction,
        )
        grouped[key].append(row)

    baseline_lookup: dict[tuple[str, str, str, str, str], str | None] = {}
    for key, group_rows in grouped.items():
        baseline_lookup[key] = choose_baseline(
            {item["case"] for item in group_rows}, key[-1]
        )

    enriched_rows: list[dict[str, Any]] = []
    for row in rows:
        direction = case_direction(row["case"])
        key = (
            row["language"],
            row["variant"],
            row["shape"],
            row["size"],
            direction,
        )
        baseline_case = baseline_lookup.get(key)
        baseline_mean = None
        ratio = None
        if baseline_case is not None:
            for candidate in grouped[key]:
                if candidate["case"] == baseline_case:
                    baseline_mean = candidate["stats"]["mean_ms"]
                    break
        if baseline_mean is not None:
            ratio = row["stats"]["mean_ms"] / baseline_mean

        enriched = dict(row)
        enriched["direction"] = direction
        enriched["baseline_case"] = baseline_case
        enriched["baseline_mean_ms"] = baseline_mean
        enriched["ratio_to_baseline"] = ratio
        enriched_rows.append(enriched)

    return {
        "benchmark_release": release_metadata.benchmark_release,
        "dataset_release": release_metadata.dataset_release,
        "generated_at": utc_now_iso(),
        "result_count": len(enriched_rows),
        "languages": [payload.get("language", "unknown") for payload in raw_payloads],
        "issues": {
            payload.get("language", "unknown"): payload.get("issues", [])
            for payload in raw_payloads
        },
        "environment": {
            payload.get("language", "unknown"): payload.get("environment", {})
            for payload in raw_payloads
        },
        "results": enriched_rows,
    }


def write_summary_csv(path: Path, summary: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fieldnames = [
        "benchmark_release",
        "dataset_release",
        "language",
        "variant",
        "shape",
        "size",
        "case",
        "direction",
        "warmups",
        "iterations",
        "mean_ms",
        "median_ms",
        "min_ms",
        "max_ms",
        "stdev_ms",
        "baseline_case",
        "baseline_mean_ms",
        "ratio_to_baseline",
    ]
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames)
        writer.writeheader()
        for row in summary["results"]:
            writer.writerow(
                {
                    "benchmark_release": row["benchmark_release"],
                    "dataset_release": row["dataset_release"],
                    "language": row["language"],
                    "variant": row["variant"],
                    "shape": row["shape"],
                    "size": row["size"],
                    "case": row["case"],
                    "direction": row["direction"],
                    "warmups": row["warmups"],
                    "iterations": row["iterations"],
                    "mean_ms": row["stats"]["mean_ms"],
                    "median_ms": row["stats"]["median_ms"],
                    "min_ms": row["stats"]["min_ms"],
                    "max_ms": row["stats"]["max_ms"],
                    "stdev_ms": row["stats"]["stdev_ms"],
                    "baseline_case": row["baseline_case"],
                    "baseline_mean_ms": row["baseline_mean_ms"],
                    "ratio_to_baseline": row["ratio_to_baseline"],
                }
            )


def render_report(summary: dict[str, Any]) -> str:
    lines = [
        "# Benchmark Report",
        "",
        f"- benchmark_release: {summary['benchmark_release']}",
        f"- dataset_release: {summary['dataset_release']}",
        f"- generated_at: {summary['generated_at']}",
        f"- result_count: {summary['result_count']}",
        "",
        "## Environment",
        "",
    ]

    for language, environment in summary["environment"].items():
        lines.append(f"### {language}")
        if not environment:
            lines.append("- no environment info")
        else:
            for key in (
                "os",
                "os_release",
                "architecture",
                "cpu_model",
                "python_version",
                "node_version",
                "rust_version",
            ):
                value = environment.get(key)
                lines.append(f"- {key}: {value if value is not None else 'unknown'}")
        lines.append("")

    if not summary["results"]:
        lines.extend(
            [
                "## Results",
                "",
                "No benchmark results to summarize.",
                "",
            ]
        )
    else:
        lines.extend(
            [
                "## Results",
                "",
                "| language | variant | shape | size | case | mean_ms | baseline | ratio |",
                "| --- | --- | --- | --- | --- | ---: | --- | ---: |",
            ]
        )
        for row in summary["results"]:
            ratio = row["ratio_to_baseline"]
            ratio_text = "-" if ratio is None else f"{ratio:.3f}"
            baseline_case = row["baseline_case"] or "-"
            lines.append(
                f"| {row['language']} | {row['variant']} | {row['shape']} | {row['size']} | "
                f"{row['case']} | {row['stats']['mean_ms']:.3f} | {baseline_case} | {ratio_text} |"
            )

        lines.append("")

    lines.append("## Issues")
    lines.append("")
    has_issues = False
    for language, issues in summary["issues"].items():
        if not issues:
            continue
        has_issues = True
        lines.append(f"### {language}")
        for issue in issues:
            lines.append(f"- {issue}")
        lines.append("")
    if not has_issues:
        lines.append("No runner issues.")
        lines.append("")

    return "\n".join(lines)


def _read_command_output(command: list[str]) -> str | None:
    try:
        completed = subprocess.run(
            command,
            capture_output=True,
            check=True,
            text=True,
        )
    except (FileNotFoundError, subprocess.CalledProcessError):
        return None
    return completed.stdout.strip() or None
