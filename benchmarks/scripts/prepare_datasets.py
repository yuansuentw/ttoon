from __future__ import annotations

import argparse
from contextlib import ExitStack
import hashlib
import json
import shutil
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PYTHON_ROOT = ROOT / "python"
for path in (ROOT, PYTHON_ROOT):
    path_str = str(path)
    if path_str not in sys.path:
        sys.path.insert(0, path_str)

from benchmarks.python.common.dataset_generation import (  # noqa: E402
    SIZE_TO_ROW_COUNT,
    build_bundle_paths,
    create_archive,
    generate_bundle,
    unpack_archive,
)
from benchmarks.python.common.datasets import (  # noqa: E402
    PreparedDataset,
    discover_prepared_datasets,
    load_prepared_dataset,
    validate_bundle_layout,
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate or validate prepared benchmark datasets")
    parser.add_argument("--source-root", type=Path, default=ROOT / "benchmarks/datasets/source")
    parser.add_argument("--prepared-root", type=Path, default=ROOT / "benchmarks/datasets/prepared")
    parser.add_argument("--variant", choices=("js-basic", "extended"))
    parser.add_argument("--size", choices=tuple(SIZE_TO_ROW_COUNT.keys()))
    parser.add_argument("--overwrite", action="store_true")
    parser.add_argument("--no-archive", action="store_true")
    parser.add_argument("--archive-only", action="store_true")
    parser.add_argument("--validate-only", action="store_true")
    parser.add_argument("--verify-reproducibility", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    selected_variants = [args.variant] if args.variant else ["js-basic", "extended"]
    selected_sizes = [args.size] if args.size else list(SIZE_TO_ROW_COUNT.keys())
    prepared_at = datetime.now(timezone.utc).isoformat()

    generated: list[dict[str, str]] = []
    archived: list[str] = []
    verified: list[dict[str, object]] = []
    issues: list[str] = []

    if not args.validate_only:
        for variant in selected_variants:
            for size in selected_sizes:
                meta = generate_bundle(
                    variant=variant,
                    size=size,
                    source_root=args.source_root,
                    prepared_root=args.prepared_root,
                    prepared_at=prepared_at,
                    overwrite=args.overwrite,
                )
                paths = build_bundle_paths(args.prepared_root, variant, size)
                generated.append(
                    {
                        "variant": variant,
                        "size": size,
                        "bundle": _display_path(paths.root),
                        "generator_version": meta["generator_version"],
                    }
                )

                if not args.no_archive:
                    create_archive(paths.root, paths.archive_path)
                    archived.append(_display_path(paths.archive_path))
                    if args.archive_only:
                        shutil.rmtree(paths.root)

    with ExitStack() as exit_stack:
        validation_targets = _collect_validation_targets(
            args.prepared_root,
            variants=selected_variants,
            sizes=selected_sizes,
            exit_stack=exit_stack,
        )
        for bundle, archive_path in validation_targets:
            issues.extend(validate_bundle_layout(bundle))
            if args.verify_reproducibility:
                record, bundle_issues = _verify_reproducibility(
                    bundle,
                    source_root=args.source_root,
                    archive_path=archive_path,
                )
                verified.append(record)
                issues.extend(bundle_issues)

    payload = {
        "generated": generated,
        "archived": archived,
        "verified": verified,
        "validated_bundle_count": len(validation_targets),
        "issues": issues,
        "row_sizes": {key: value for key, value in SIZE_TO_ROW_COUNT.items() if key in selected_sizes},
    }
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0 if not issues else 1


def _verify_reproducibility(
    bundle: PreparedDataset,
    *,
    source_root: Path,
    archive_path: Path | None,
) -> tuple[dict[str, object], list[str]]:
    issues: list[str] = []
    with tempfile.TemporaryDirectory(prefix="benchmark-repro-") as tmp_dir:
        temp_prepared_root = Path(tmp_dir) / "prepared"
        generated_meta = generate_bundle(
            variant=bundle.variant,
            size=bundle.size,
            source_root=source_root,
            prepared_root=temp_prepared_root,
            prepared_at=bundle.prepared_at,
            overwrite=True,
        )
        temp_paths = build_bundle_paths(temp_prepared_root, bundle.variant, bundle.size)
        create_archive(temp_paths.root, temp_paths.archive_path)

        current_hashes = _collect_file_hashes(bundle.root)
        regenerated_hashes = _collect_file_hashes(temp_paths.root)
        content_hash = _fold_hashes(current_hashes)
        regenerated_content_hash = _fold_hashes(regenerated_hashes)
        content_match = current_hashes == regenerated_hashes
        if not content_match:
            issues.append(f"{bundle.meta_path}: reproducibility content hash mismatch")

        meta_match = bundle.raw_meta == generated_meta
        if not meta_match:
            issues.append(f"{bundle.meta_path}: reproducibility meta mismatch")

        archive_exists = archive_path.is_file() if archive_path else False
        archive_hash = _sha256_file(archive_path) if archive_exists and archive_path else None
        regenerated_archive_hash = _sha256_file(temp_paths.archive_path)
        archive_match = archive_exists and archive_hash == regenerated_archive_hash
        if not archive_exists:
            archive_name = archive_path.name if archive_path else f"{bundle.size}.tar.zst"
            issues.append(f"{bundle.meta_path}: missing archive {archive_name}")
        elif not archive_match:
            issues.append(f"{bundle.meta_path}: reproducibility archive hash mismatch")

    return (
        {
            "variant": bundle.variant,
            "size": bundle.size,
            "meta_path": _display_path(bundle.meta_path),
            "content_hash": content_hash,
            "regenerated_content_hash": regenerated_content_hash,
            "content_match": content_match,
            "archive_present": archive_exists,
            "archive_hash": archive_hash,
            "regenerated_archive_hash": regenerated_archive_hash,
            "archive_match": archive_match,
            "meta_match": meta_match,
        },
        issues,
    )


def _collect_file_hashes(root: Path) -> dict[str, str]:
    return {
        str(path.relative_to(root)): _sha256_file(path)
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


def _fold_hashes(hashes: dict[str, str]) -> str:
    digest = hashlib.sha256()
    for relative_path, file_hash in sorted(hashes.items()):
        digest.update(relative_path.encode("utf-8"))
        digest.update(b"\0")
        digest.update(file_hash.encode("ascii"))
        digest.update(b"\n")
    return digest.hexdigest()


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def _display_path(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(ROOT))
    except ValueError:
        return str(path)


def _collect_validation_targets(
    prepared_root: Path,
    *,
    variants: list[str],
    sizes: list[str],
    exit_stack: ExitStack,
) -> list[tuple[PreparedDataset, Path | None]]:
    discovered = {
        (bundle.variant, bundle.size): bundle
        for bundle in discover_prepared_datasets(prepared_root)
    }
    targets: list[tuple[PreparedDataset, Path | None]] = []
    for variant in variants:
        for size in sizes:
            archive_path = build_bundle_paths(prepared_root, variant, size).archive_path
            key = (variant, size)
            if key in discovered:
                targets.append((discovered[key], archive_path if archive_path.is_file() else None))
                continue
            if not archive_path.is_file():
                continue

            temp_root = Path(exit_stack.enter_context(tempfile.TemporaryDirectory(prefix="benchmark-validate-")))
            unpacked_root = unpack_archive(archive_path, temp_root / variant, overwrite=True)
            targets.append(
                (
                    load_prepared_dataset(
                        unpacked_root / "meta.json",
                        dataset_root=unpacked_root,
                        display_meta_path=prepared_root / variant / size / "meta.json",
                    ),
                    archive_path,
                )
            )
    return targets


if __name__ == "__main__":
    raise SystemExit(main())
