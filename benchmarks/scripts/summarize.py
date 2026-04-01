from __future__ import annotations

import argparse
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PYTHON_ROOT = ROOT / "python"
for path in (ROOT, PYTHON_ROOT):
    path_str = str(path)
    if path_str not in sys.path:
        sys.path.insert(0, path_str)

from benchmarks.python.common.reporting import (  # noqa: E402
    build_summary,
    load_json,
    render_report,
    write_json,
    write_summary_csv,
)
from benchmarks.python.common.manifests import (
    load_release_metadata,
    validate_raw_payload_release,
)  # noqa: E402


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Summarize benchmark raw outputs")
    parser.add_argument("--results-dir", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    release_metadata = load_release_metadata()
    results_dir = (
        args.results_dir.resolve()
        if args.results_dir
        else _latest_results_dir((ROOT / "benchmarks/results").resolve())
    )
    raw_dir = results_dir / "raw"
    payloads = []
    for raw_file in sorted(raw_dir.glob("*.json")):
        payload = load_json(raw_file)
        validate_raw_payload_release(
            payload, release_metadata, source_label=_display_path(raw_file)
        )
        payloads.append(payload)

    summary = build_summary(payloads, release_metadata)
    write_json(results_dir / "summary.json", summary)
    write_summary_csv(results_dir / "summary.csv", summary)
    (results_dir / "report.md").write_text(render_report(summary), encoding="utf-8")
    print(_display_path(results_dir))
    return 0


def _latest_results_dir(results_root: Path) -> Path:
    candidates = [
        path for path in results_root.iterdir() if path.is_dir() and path.name != "raw"
    ]
    if not candidates:
        raise SystemExit("no result directories found under benchmarks/results")
    return sorted(candidates)[-1]


def _display_path(path: Path) -> str:
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


if __name__ == "__main__":
    raise SystemExit(main())
