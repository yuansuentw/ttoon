from __future__ import annotations

import shlex
from dataclasses import dataclass
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[3]
BENCHMARK_MANIFEST_PATH = ROOT / "benchmarks/manifests/benchmark_release.sh"
DATASET_MANIFEST_PATH = ROOT / "benchmarks/manifests/datasets.sh"


@dataclass(frozen=True)
class BenchmarkReleaseManifest:
    manifest_version: int
    benchmark_release: str
    benchmark_dataset_release: int
    languages: tuple[str, ...]
    variants: tuple[str, ...]
    sizes: tuple[str, ...]
    notes_ref: str
    storage_provider: str
    default_dataset_root: str


@dataclass(frozen=True)
class DatasetManifest:
    manifest_version: int
    dataset_release: int
    storage_provider: str
    base_url: str
    sha256_by_key: dict[str, str]

    def build_remote_url(self, variant: str, size: str) -> str | None:
        if not self.base_url:
            return None
        return f"{self.base_url.rstrip('/')}/datasets/releases/{self.dataset_release}/{variant}/{size}.tar.zst"

    def archive_key(self, variant: str, size: str) -> str:
        return f"{variant}/{size}"

    def expected_sha256(self, variant: str, size: str) -> str | None:
        return self.sha256_by_key.get(self.archive_key(variant, size))


@dataclass(frozen=True)
class ReleaseMetadata:
    benchmark_manifest: BenchmarkReleaseManifest
    dataset_manifest: DatasetManifest

    @property
    def benchmark_release(self) -> str:
        return self.benchmark_manifest.benchmark_release

    @property
    def dataset_release(self) -> int:
        return self.dataset_manifest.dataset_release


def load_release_metadata(
    *,
    benchmark_manifest_path: Path = BENCHMARK_MANIFEST_PATH,
    dataset_manifest_path: Path = DATASET_MANIFEST_PATH,
) -> ReleaseMetadata:
    benchmark_manifest = load_benchmark_release_manifest(benchmark_manifest_path)
    dataset_manifest = load_dataset_manifest(dataset_manifest_path)

    benchmark_release_major = parse_benchmark_release_major(
        benchmark_manifest.benchmark_release
    )
    benchmark_dataset_release = benchmark_manifest.benchmark_dataset_release
    dataset_release = dataset_manifest.dataset_release

    if benchmark_release_major != benchmark_dataset_release:
        raise ValueError(
            "BENCHMARK_RELEASE does not match BENCHMARK_DATASET_RELEASE: "
            f"{benchmark_manifest.benchmark_release} vs {benchmark_manifest.benchmark_dataset_release}"
        )
    if benchmark_dataset_release != dataset_release:
        raise ValueError(
            "BENCHMARK_DATASET_RELEASE does not match DATASET_RELEASE: "
            f"{benchmark_manifest.benchmark_dataset_release} vs {dataset_manifest.dataset_release}"
        )

    return ReleaseMetadata(
        benchmark_manifest=benchmark_manifest,
        dataset_manifest=dataset_manifest,
    )


def load_benchmark_release_manifest(
    path: Path = BENCHMARK_MANIFEST_PATH,
) -> BenchmarkReleaseManifest:
    assignments = _parse_shell_assignments(path)
    return BenchmarkReleaseManifest(
        manifest_version=int(
            _require_scalar(assignments, "BENCHMARK_MANIFEST_VERSION", path)
        ),
        benchmark_release=_require_scalar(assignments, "BENCHMARK_RELEASE", path),
        benchmark_dataset_release=_parse_dataset_release(
            _require_scalar(assignments, "BENCHMARK_DATASET_RELEASE", path)
        ),
        languages=_require_array(assignments, "BENCHMARK_LANGUAGES", path),
        variants=_require_array(assignments, "BENCHMARK_VARIANTS", path),
        sizes=_require_array(assignments, "BENCHMARK_SIZES", path),
        notes_ref=_require_scalar(assignments, "BENCHMARK_NOTES_REF", path),
        storage_provider=_require_scalar(
            assignments, "BENCHMARK_STORAGE_PROVIDER", path
        ),
        default_dataset_root=_require_scalar(
            assignments, "BENCHMARK_DEFAULT_DATASET_ROOT", path
        ),
    )


def load_dataset_manifest(path: Path = DATASET_MANIFEST_PATH) -> DatasetManifest:
    content = path.read_text(encoding="utf-8")
    assignments = _parse_shell_assignments_from_text(content)
    return DatasetManifest(
        manifest_version=int(
            _require_scalar(assignments, "DATASET_MANIFEST_VERSION", path)
        ),
        dataset_release=_parse_dataset_release(
            _require_scalar(assignments, "DATASET_RELEASE", path)
        ),
        storage_provider=_require_scalar(assignments, "DATASET_STORAGE_PROVIDER", path),
        base_url=_require_scalar(assignments, "DATASET_BASE_URL", path),
        sha256_by_key=_parse_dataset_sha256_map(content),
    )


def parse_benchmark_release_major(benchmark_release: str) -> int:
    parts = benchmark_release.split(".")
    if len(parts) != 2 or not all(part.isdigit() for part in parts):
        raise ValueError(f"invalid BENCHMARK_RELEASE: {benchmark_release}")
    return int(parts[0])


def validate_raw_payload_release(
    payload: dict[str, Any],
    release_metadata: ReleaseMetadata,
    *,
    source_label: str,
) -> None:
    actual_benchmark_release = payload.get("benchmark_release")
    actual_dataset_release = payload.get("dataset_release")

    if actual_benchmark_release != release_metadata.benchmark_release:
        raise ValueError(
            f"{source_label}: benchmark_release mismatch, "
            f"expected {release_metadata.benchmark_release}, got {actual_benchmark_release!r}"
        )

    if actual_dataset_release != release_metadata.dataset_release:
        raise ValueError(
            f"{source_label}: dataset_release mismatch, "
            f"expected {release_metadata.dataset_release}, got {actual_dataset_release!r}"
        )


def _parse_shell_assignments(path: Path) -> dict[str, str | tuple[str, ...]]:
    return _parse_shell_assignments_from_text(path.read_text(encoding="utf-8"))


def _parse_shell_assignments_from_text(
    content: str,
) -> dict[str, str | tuple[str, ...]]:
    assignments: dict[str, str | tuple[str, ...]] = {}
    lines = content.splitlines()
    index = 0

    while index < len(lines):
        line = lines[index].strip()
        index += 1

        if not line or line.startswith("#"):
            continue
        if line.startswith("declare -A "):
            while index < len(lines) and lines[index].strip() != ")":
                index += 1
            if index < len(lines):
                index += 1
            continue
        if "=" not in line:
            continue

        name, raw_value = line.split("=", 1)
        name = name.strip()
        raw_value = raw_value.strip()
        if not name:
            continue

        if raw_value.startswith("("):
            array_lines = [raw_value]
            while not array_lines[-1].endswith(")") and index < len(lines):
                array_lines.append(lines[index].strip())
                index += 1
            assignments[name] = _parse_shell_array(" ".join(array_lines))
            continue

        assignments[name] = _parse_shell_scalar(raw_value)

    return assignments


def _parse_dataset_sha256_map(content: str) -> dict[str, str]:
    entries: dict[str, str] = {}
    lines = content.splitlines()
    in_block = False

    for raw_line in lines:
        line = raw_line.strip()
        if not in_block:
            if line.startswith("declare -A DATASET_SHA256="):
                in_block = True
            continue

        if line == ")":
            break
        if not line or line.startswith("#"):
            continue
        if not line.startswith('["') or '"]="' not in line or not line.endswith('"'):
            continue

        key_part, value_part = line.split('"]="', 1)
        key = key_part[2:]
        value = value_part[:-1]
        entries[key] = value

    return entries


def _parse_shell_scalar(raw_value: str) -> str:
    values = shlex.split(raw_value, posix=True)
    if len(values) != 1:
        raise ValueError(f"unsupported shell scalar: {raw_value}")
    return values[0]


def _parse_shell_array(raw_value: str) -> tuple[str, ...]:
    if not raw_value.endswith(")"):
        raise ValueError(f"malformed shell array: {raw_value}")
    body = raw_value[1:-1].strip()
    if not body:
        return ()
    return tuple(shlex.split(body, posix=True))


def _require_scalar(
    assignments: dict[str, str | tuple[str, ...]],
    key: str,
    path: Path,
) -> str:
    value = assignments.get(key)
    if value is None:
        raise ValueError(f"{path} missing required field: {key}")
    if not isinstance(value, str):
        raise ValueError(f"{path} type error, expected scalar: {key}")
    return value


def _require_array(
    assignments: dict[str, str | tuple[str, ...]],
    key: str,
    path: Path,
) -> tuple[str, ...]:
    value = assignments.get(key)
    if value is None:
        raise ValueError(f"{path} missing required field: {key}")
    if not isinstance(value, tuple):
        raise ValueError(f"{path} type error, expected array: {key}")
    return value


def _parse_dataset_release(dataset_release: str) -> int:
    if not dataset_release.isdigit():
        raise ValueError(f"invalid dataset_release: {dataset_release}")
    return int(dataset_release)
