from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
import tempfile
from typing import Any
from urllib.request import urlopen

from benchmarks.python.common.config import SIZES, VARIANTS
from benchmarks.python.common.manifests import load_release_metadata


@dataclass(frozen=True)
class PreparedDataset:
    root: Path
    meta_path: Path
    variant: str
    size: str
    schema_version: int
    row_count: int
    schema_summary: dict[str, Any]
    source_provenance: dict[str, Any]
    generator_version: str
    prepared_at: str
    available_benchmark_targets: dict[str, Any]
    raw_meta: dict[str, Any]

    @property
    def structure_dir(self) -> Path:
        return self.root / "structure"

    @property
    def tabular_dir(self) -> Path:
        return self.root / "tabular"

    def file_for(self, shape: str, name: str) -> Path:
        if shape == "structure":
            return self.structure_dir / name
        if shape == "tabular":
            return self.tabular_dir / name
        raise ValueError(f"unknown shape: {shape}")

    def has_target(self, shape: str, key: str) -> bool:
        section = self.available_benchmark_targets.get(shape, {})
        return bool(section.get(key, False))


def discover_prepared_dataset_archives(
    prepared_root: Path,
    *,
    variant: str | None = None,
    size: str | None = None,
) -> list[Path]:
    variant_filters = (variant,) if variant else VARIANTS
    size_filters = (size,) if size else SIZES
    archives: list[Path] = []
    for variant_name in variant_filters:
        for size_name in size_filters:
            archive_path = prepared_root / variant_name / f"{size_name}.tar.zst"
            if archive_path.is_file():
                archives.append(archive_path)
    return archives


def discover_prepared_datasets(
    prepared_root: Path,
    *,
    variant: str | None = None,
    size: str | None = None,
) -> list[PreparedDataset]:
    datasets: list[PreparedDataset] = []
    if not prepared_root.exists():
        return datasets

    variant_filters = {variant} if variant else set(VARIANTS)
    size_filters = {size} if size else set(SIZES)

    for meta_path in sorted(prepared_root.glob("*/*/meta.json")):
        bundle = load_prepared_dataset(meta_path)
        if bundle.variant not in variant_filters:
            continue
        if bundle.size not in size_filters:
            continue
        datasets.append(bundle)
    return datasets


def ensure_prepared_datasets(
    prepared_root: Path,
    *,
    variant: str | None = None,
    size: str | None = None,
    overwrite: bool = False,
) -> tuple[list[PreparedDataset], list[Path]]:
    variant_filters = (variant,) if variant else VARIANTS
    size_filters = (size,) if size else SIZES
    unpacked: list[Path] = []
    dataset_manifest = load_release_metadata().dataset_manifest

    for variant_name in variant_filters:
        for size_name in size_filters:
            meta_path = prepared_root / variant_name / size_name / "meta.json"
            if meta_path.is_file():
                continue

            archive_path = prepared_root / variant_name / f"{size_name}.tar.zst"
            expected_sha256 = dataset_manifest.expected_sha256(variant_name, size_name)
            if archive_path.is_file():
                _verify_archive_hash(archive_path, expected_sha256)
            else:
                remote_url = dataset_manifest.build_remote_url(variant_name, size_name)
                if remote_url is None:
                    continue
                _download_archive(
                    remote_url,
                    archive_path,
                    expected_sha256,
                )

            from benchmarks.python.common.dataset_generation import unpack_archive

            unpacked.append(
                unpack_archive(archive_path, archive_path.parent, overwrite=overwrite)
            )

    return (
        discover_prepared_datasets(
            prepared_root,
            variant=variant,
            size=size,
        ),
        unpacked,
    )


def load_prepared_dataset(
    meta_path: Path,
    *,
    dataset_root: Path | None = None,
    display_meta_path: Path | None = None,
) -> PreparedDataset:
    payload = json.loads(meta_path.read_text(encoding="utf-8"))
    required = (
        "schema_version",
        "variant",
        "size",
        "row_count",
        "schema_summary",
        "source_provenance",
        "generator_version",
        "prepared_at",
        "available_benchmark_targets",
    )
    missing = [field for field in required if field not in payload]
    if missing:
        raise ValueError(f"{meta_path} missing required fields: {', '.join(missing)}")

    return PreparedDataset(
        root=dataset_root or meta_path.parent,
        meta_path=display_meta_path or meta_path,
        variant=payload["variant"],
        size=payload["size"],
        schema_version=int(payload["schema_version"]),
        row_count=int(payload["row_count"]),
        schema_summary=dict(payload["schema_summary"]),
        source_provenance=dict(payload["source_provenance"]),
        generator_version=str(payload["generator_version"]),
        prepared_at=str(payload["prepared_at"]),
        available_benchmark_targets=dict(payload["available_benchmark_targets"]),
        raw_meta=payload,
    )


def validate_bundle_layout(bundle: PreparedDataset) -> list[str]:
    issues: list[str] = []
    if (
        bundle.has_target("structure", "source_json")
        and not bundle.file_for("structure", "source.json").is_file()
    ):
        issues.append(f"{bundle.meta_path}: missing structure/source.json")
    if (
        bundle.has_target("structure", "tjson")
        and not bundle.file_for("structure", "tjson.txt").is_file()
    ):
        issues.append(f"{bundle.meta_path}: missing structure/tjson.txt")
    if (
        bundle.has_target("structure", "ttoon")
        and not bundle.file_for("structure", "ttoon.txt").is_file()
    ):
        issues.append(f"{bundle.meta_path}: missing structure/ttoon.txt")
    if (
        bundle.has_target("structure", "toon")
        and not bundle.file_for("structure", "toon.txt").is_file()
    ):
        issues.append(f"{bundle.meta_path}: missing structure/toon.txt")
    if (
        bundle.has_target("tabular", "source_arrow")
        and not bundle.file_for("tabular", "source.arrow").is_file()
    ):
        issues.append(f"{bundle.meta_path}: missing tabular/source.arrow")
    if (
        bundle.has_target("tabular", "tjson")
        and not bundle.file_for("tabular", "tjson.txt").is_file()
    ):
        issues.append(f"{bundle.meta_path}: missing tabular/tjson.txt")
    if (
        bundle.has_target("tabular", "ttoon")
        and not bundle.file_for("tabular", "ttoon.txt").is_file()
    ):
        issues.append(f"{bundle.meta_path}: missing tabular/ttoon.txt")
    return issues


def _download_archive(
    remote_url: str, archive_path: Path, expected_sha256: str | None
) -> None:
    archive_path.parent.mkdir(parents=True, exist_ok=True)
    with (
        urlopen(remote_url) as response,
        tempfile.NamedTemporaryFile(
            dir=archive_path.parent,
            delete=False,
            prefix=f".{archive_path.name}.",
            suffix=".tmp",
        ) as tmp_file,
    ):
        temp_path = Path(tmp_file.name)
        try:
            while True:
                chunk = response.read(1024 * 1024)
                if not chunk:
                    break
                tmp_file.write(chunk)
        except Exception:
            temp_path.unlink(missing_ok=True)
            raise

    try:
        _verify_archive_hash(temp_path, expected_sha256)
        temp_path.replace(archive_path)
    except Exception:
        temp_path.unlink(missing_ok=True)
        raise


def _verify_archive_hash(archive_path: Path, expected_sha256: str | None) -> None:
    if not expected_sha256:
        return
    actual_sha256 = _sha256_file(archive_path)
    if actual_sha256 != expected_sha256:
        raise ValueError(
            f"{archive_path}: archive hash mismatch, expected {expected_sha256}, got {actual_sha256}"
        )


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()
