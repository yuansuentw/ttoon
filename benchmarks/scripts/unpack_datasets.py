from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PYTHON_ROOT = ROOT / "python"
for path in (ROOT, PYTHON_ROOT):
    path_str = str(path)
    if path_str not in sys.path:
        sys.path.insert(0, path_str)

from benchmarks.python.common.dataset_generation import SIZE_TO_ROW_COUNT, unpack_archive  # noqa: E402


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Unpack benchmark dataset archives")
    parser.add_argument("--prepared-root", type=Path, default=ROOT / "benchmarks/datasets/prepared")
    parser.add_argument("--variant", choices=("js-basic", "extended"))
    parser.add_argument("--size", choices=tuple(SIZE_TO_ROW_COUNT.keys()))
    parser.add_argument("--overwrite", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    archives = _discover_archives(args.prepared_root, args.variant, args.size)
    unpacked: list[str] = []
    for archive in archives:
        target = unpack_archive(archive, archive.parent, overwrite=args.overwrite)
        unpacked.append(_display_path(target))

    payload = {
        "archive_count": len(archives),
        "archives": [_display_path(path) for path in archives],
        "unpacked": unpacked,
    }
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


def _discover_archives(prepared_root: Path, variant: str | None, size: str | None) -> list[Path]:
    variants = [variant] if variant else ["js-basic", "extended"]
    sizes = [size] if size else list(SIZE_TO_ROW_COUNT.keys())
    archives: list[Path] = []
    for variant_name in variants:
        for size_name in sizes:
            archive = prepared_root / variant_name / f"{size_name}.tar.zst"
            if archive.is_file():
                archives.append(archive)
    return archives


def _display_path(path: Path) -> str:
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


if __name__ == "__main__":
    raise SystemExit(main())
