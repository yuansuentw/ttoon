from __future__ import annotations

import argparse
import json
import subprocess
import sys
import threading
import time
from datetime import datetime
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
PYTHON_ROOT = ROOT / "python"
for path in (ROOT, PYTHON_ROOT):
    path_str = str(path)
    if path_str not in sys.path:
        sys.path.insert(0, path_str)

from benchmarks.python.common.config import CASE_MATRIX, DEFAULT_ITERATIONS, DEFAULT_WARMUPS, list_languages  # noqa: E402
from benchmarks.python.common.datasets import discover_prepared_datasets, ensure_prepared_datasets  # noqa: E402
from benchmarks.python.common.manifests import load_release_metadata, validate_raw_payload_release  # noqa: E402
from benchmarks.python.common.reporting import (  # noqa: E402
    build_summary,
    render_report,
    write_json,
    write_summary_csv,
)


class _ProgressTracker:
    """Displays 3-line progress to stderr while benchmarks run."""

    def __init__(self, total: int) -> None:
        self._total = total
        self._completed = 0
        self._current_label = ""
        self._start_time = 0.0
        self._prev_label = ""
        self._prev_ms: float | None = None
        self._running = False
        self._thread: threading.Thread | None = None
        self._lock = threading.Lock()
        self._first_render = True
        self._is_tty = sys.stderr.isatty()

    def start_case(self, label: str) -> None:
        with self._lock:
            self._current_label = label
            self._start_time = time.monotonic()
        self._running = True
        self._thread = threading.Thread(target=self._tick, daemon=True)
        self._thread.start()

    def finish_case(self, mean_ms: float | None = None) -> None:
        self._running = False
        if self._thread:
            self._thread.join(timeout=2)
        with self._lock:
            self._completed += 1
            self._prev_label = self._current_label
            self._prev_ms = mean_ms
        self._render()

    def close(self) -> None:
        self._running = False
        if self._thread:
            self._thread.join(timeout=2)
        if self._is_tty and not self._first_render:
            sys.stderr.write("\n")
            sys.stderr.flush()

    def _tick(self) -> None:
        while self._running:
            self._render()
            time.sleep(0.5)

    def _render(self) -> None:
        if not self._is_tty:
            return
        with self._lock:
            pct = (self._completed / self._total * 100) if self._total else 0
            bar_w = 30
            filled = int(bar_w * self._completed / self._total) if self._total else 0
            bar = "\u2588" * filled + "\u2591" * (bar_w - filled)
            elapsed = time.monotonic() - self._start_time if self._start_time else 0
            elapsed_str = f"{elapsed:.1f}s"

            line1 = f"  [{bar}] {pct:5.1f}%  ({self._completed}/{self._total})"

            if self._prev_ms is not None:
                line2 = f"  prev: {self._prev_label} \u2014 {self._prev_ms:.2f} ms"
            elif self._prev_label:
                line2 = f"  prev: {self._prev_label} \u2014 failed"
            else:
                line2 = "  prev: \u2014"

            line3 = f"  now:  {self._current_label} \u2014 {elapsed_str}"

            if self._first_render:
                sys.stderr.write(f"{line1}\n{line2}\n{line3}")
                self._first_render = False
            else:
                sys.stderr.write(f"\033[2A\r\033[K{line1}\n\033[K{line2}\n\033[K{line3}")
            sys.stderr.flush()


def _count_all_targets(
    languages: list[str],
    args: argparse.Namespace,
    discovered: list[Any],
) -> int:
    total = 0
    for language in languages:
        for dataset in discovered:
            matrix = CASE_MATRIX.get(language, {}).get(dataset.variant, {})
            for shape, cases in matrix.items():
                if args.shape and shape != args.shape:
                    continue
                for case_name in cases:
                    if args.case and case_name != args.case:
                        continue
                    total += 1
    return total


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run all benchmark runners")
    parser.add_argument("--language", choices=("python", "js", "rust"))
    parser.add_argument("--variant", choices=("js-basic", "extended"))
    parser.add_argument("--size", choices=("10k", "100k", "1m"))
    parser.add_argument("--shape", choices=("structure", "tabular"))
    parser.add_argument("--case")
    parser.add_argument("--warmups", type=int, default=DEFAULT_WARMUPS)
    parser.add_argument("--iterations", type=int, default=DEFAULT_ITERATIONS)
    parser.add_argument("--dataset-root", type=Path, default=ROOT / "benchmarks/datasets/prepared")
    parser.add_argument("--results-root", type=Path, default=ROOT / "benchmarks/results")
    parser.add_argument("--trace-memory", action="store_true")
    parser.add_argument("--no-auto-unpack", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    release_metadata = load_release_metadata()
    timestamp = datetime.now().astimezone().strftime("%Y%m%d-%H%M%S")
    output_dir = args.results_root / timestamp
    raw_dir = output_dir / "raw"
    raw_dir.mkdir(parents=True, exist_ok=True)

    if args.no_auto_unpack:
        discovered = discover_prepared_datasets(
            args.dataset_root,
            variant=args.variant,
            size=args.size,
        )
        auto_unpacked: list[Path] = []
    else:
        discovered, auto_unpacked = ensure_prepared_datasets(
            args.dataset_root,
            variant=args.variant,
            size=args.size,
        )
    languages = [args.language] if args.language else list_languages()
    tracker = _ProgressTracker(_count_all_targets(languages, args, discovered))

    raw_payloads: list[dict[str, Any]] = []
    for language in languages:
        payload = _run_language(language, args, discovered, release_metadata, tracker)
        validate_raw_payload_release(payload, release_metadata, source_label=f"runner:{language}")
        raw_payloads.append(payload)
        write_json(raw_dir / f"{language}.json", payload)
    tracker.close()

    summary = build_summary(raw_payloads, release_metadata)
    summary["requested_filters"] = {
        "language": args.language,
        "variant": args.variant,
        "size": args.size,
        "shape": args.shape,
        "case": args.case,
        "warmups": args.warmups,
        "iterations": args.iterations,
    }
    summary["discovered_datasets"] = [
        {
            "variant": dataset.variant,
            "size": dataset.size,
            "row_count": dataset.row_count,
            "meta_path": _display_path(dataset.meta_path),
        }
        for dataset in discovered
    ]
    summary["auto_unpacked"] = [_display_path(path) for path in auto_unpacked]

    write_json(output_dir / "summary.json", summary)
    write_summary_csv(output_dir / "summary.csv", summary)
    (output_dir / "report.md").write_text(render_report(summary), encoding="utf-8")

    print(json.dumps({"results_dir": _display_path(output_dir)}, ensure_ascii=False, indent=2))
    return 0


def _run_language(
    language: str,
    args: argparse.Namespace,
    discovered: list[Any],
    release_metadata: Any,
    tracker: _ProgressTracker,
) -> dict[str, Any]:
    targets: list[tuple[str, str, str, str]] = []
    for dataset in discovered:
        matrix = CASE_MATRIX.get(language, {}).get(dataset.variant, {})
        for shape, cases in matrix.items():
            if args.shape and shape != args.shape:
                continue
            for case_name in cases:
                if args.case and case_name != args.case:
                    continue
                targets.append((dataset.variant, dataset.size, shape, case_name))

    if not targets:
        return _error_payload(language, discovered, "no matching cases", release_metadata)

    merged_payload: dict[str, Any] | None = None
    all_results: list[dict[str, Any]] = []
    all_issues: list[str] = []

    for variant, size, shape, case_name in targets:
        label = f"{language}/{variant}/{size}/{shape}/{case_name}"
        tracker.start_case(label)
        payload = _run_single_case(language, args, release_metadata, variant, size, shape, case_name)
        results = payload.get("results", [])
        mean_ms = results[0]["stats"]["mean_ms"] if results and "stats" in results[0] else None
        tracker.finish_case(mean_ms)
        if merged_payload is None:
            merged_payload = payload
        all_results.extend(results)
        all_issues.extend(payload.get("issues", []))

    assert merged_payload is not None
    merged_payload["results"] = all_results
    merged_payload["issues"] = all_issues
    merged_payload["dataset_count"] = len(discovered)
    return merged_payload


def _run_single_case(
    language: str,
    args: argparse.Namespace,
    release_metadata: Any,
    variant: str,
    size: str,
    shape: str,
    case_name: str,
) -> dict[str, Any]:
    command = _build_case_command(language, args, release_metadata, variant, size, shape, case_name)
    try:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            capture_output=True,
            check=True,
            text=True,
        )
        return json.loads(completed.stdout)
    except FileNotFoundError as exc:
        return {"results": [], "issues": [f"runner command not found: {exc}"]}
    except subprocess.CalledProcessError as exc:
        stderr = exc.stderr.strip()
        stdout = exc.stdout.strip()
        message = stderr or stdout or f"runner exited with {exc.returncode}"
        return {"results": [], "issues": [message]}
    except json.JSONDecodeError as exc:
        return {"results": [], "issues": [f"runner returned invalid JSON: {exc}"]}


def _build_case_command(
    language: str,
    args: argparse.Namespace,
    release_metadata: Any,
    variant: str,
    size: str,
    shape: str,
    case_name: str,
) -> list[str]:
    base_args = [
        "--dataset-root", str(args.dataset_root),
        "--variant", variant,
        "--size", size,
        "--shape", shape,
        "--case", case_name,
        "--warmups", str(args.warmups),
        "--iterations", str(args.iterations),
        "--benchmark-release", release_metadata.benchmark_release,
        "--dataset-release", str(release_metadata.dataset_release),
    ]
    if getattr(args, "trace_memory", False):
        base_args.append("--trace-memory")

    if language == "python":
        return [
            "uv",
            "run",
            "--project",
            str(ROOT / "python"),
            "python",
            str(ROOT / "benchmarks/python/runner.py"),
            *base_args,
        ]
    if language == "js":
        return [
            "node",
            "--max-old-space-size=10240",
            str(ROOT / "benchmarks/js/src/runner.js"),
            *base_args,
        ]
    if language == "rust":
        return [
            "cargo",
            "run",
            "--release",
            "--manifest-path",
            str(ROOT / "benchmarks/rust/Cargo.toml"),
            "--",
            *base_args,
        ]
    raise ValueError(f"unknown language: {language}")


def _error_payload(
    language: str,
    discovered: list[Any],
    message: str,
    release_metadata: Any,
) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "benchmark_release": release_metadata.benchmark_release,
        "dataset_release": release_metadata.dataset_release,
        "language": language,
        "generated_at": datetime.now().astimezone().isoformat(),
        "environment": {},
        "dataset_count": len(discovered),
        "results": [],
        "issues": [message],
    }


def _display_path(path: Path) -> str:
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


if __name__ == "__main__":
    raise SystemExit(main())
