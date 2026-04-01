from __future__ import annotations

import hashlib
import io
import sys
from pathlib import Path
from types import SimpleNamespace

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
PYTHON_ROOT = REPO_ROOT / "python"
for path in (REPO_ROOT, PYTHON_ROOT):
    path_str = str(path)
    if path_str not in sys.path:
        sys.path.insert(0, path_str)

from benchmarks.python.common import datasets as benchmark_datasets
from benchmarks.python.common.manifests import DatasetManifest, load_release_metadata
from benchmarks.python.common.reporting import build_summary


def test_load_release_metadata_parses_shell_manifests(tmp_path):
    manifests_dir = tmp_path / "benchmarks/manifests"
    manifests_dir.mkdir(parents=True)
    (manifests_dir / "benchmark_release.sh").write_text(
        "\n".join(
            [
                'BENCHMARK_MANIFEST_VERSION="1"',
                'BENCHMARK_RELEASE="7.2"',
                'BENCHMARK_DATASET_RELEASE="7"',
                'BENCHMARK_LANGUAGES=("python" "js" "rust")',
                'BENCHMARK_VARIANTS=("js-basic" "extended")',
                'BENCHMARK_SIZES=("10k" "100k" "1m")',
                'BENCHMARK_NOTES_REF="docs/dev/example.md"',
                'BENCHMARK_STORAGE_PROVIDER="local"',
                'BENCHMARK_DEFAULT_DATASET_ROOT="benchmarks/datasets/prepared"',
                "",
            ]
        ),
        encoding="utf-8",
    )
    (manifests_dir / "datasets.sh").write_text(
        "\n".join(
            [
                'DATASET_MANIFEST_VERSION="1"',
                'DATASET_RELEASE="7"',
                'DATASET_STORAGE_PROVIDER="local"',
                'DATASET_BASE_URL="https://example.com"',
                "declare -A DATASET_SHA256=(",
                '  ["js-basic/10k"]="abc123"',
                ")",
                "",
            ]
        ),
        encoding="utf-8",
    )

    metadata = load_release_metadata(
        benchmark_manifest_path=manifests_dir / "benchmark_release.sh",
        dataset_manifest_path=manifests_dir / "datasets.sh",
    )

    assert metadata.benchmark_release == "7.2"
    assert metadata.dataset_release == 7
    assert metadata.benchmark_manifest.languages == ("python", "js", "rust")
    assert metadata.dataset_manifest.base_url == "https://example.com"
    assert metadata.dataset_manifest.storage_provider == "local"
    assert metadata.dataset_manifest.sha256_by_key == {"js-basic/10k": "abc123"}
    assert metadata.dataset_manifest.expected_sha256("js-basic", "10k") == "abc123"


def test_load_release_metadata_rejects_mismatched_release_pin(tmp_path):
    manifests_dir = tmp_path / "benchmarks/manifests"
    manifests_dir.mkdir(parents=True)
    (manifests_dir / "benchmark_release.sh").write_text(
        "\n".join(
            [
                'BENCHMARK_MANIFEST_VERSION="1"',
                'BENCHMARK_RELEASE="2.0"',
                'BENCHMARK_DATASET_RELEASE="2"',
                'BENCHMARK_LANGUAGES=("python")',
                'BENCHMARK_VARIANTS=("js-basic")',
                'BENCHMARK_SIZES=("10k")',
                'BENCHMARK_NOTES_REF=""',
                'BENCHMARK_STORAGE_PROVIDER="local"',
                'BENCHMARK_DEFAULT_DATASET_ROOT="benchmarks/datasets/prepared"',
                "",
            ]
        ),
        encoding="utf-8",
    )
    (manifests_dir / "datasets.sh").write_text(
        "\n".join(
            [
                'DATASET_MANIFEST_VERSION="1"',
                'DATASET_RELEASE="3"',
                'DATASET_STORAGE_PROVIDER="local"',
                'DATASET_BASE_URL=""',
                "declare -A DATASET_SHA256=()",
                "",
            ]
        ),
        encoding="utf-8",
    )

    with pytest.raises(
        ValueError, match="BENCHMARK_DATASET_RELEASE does not match DATASET_RELEASE"
    ):
        load_release_metadata(
            benchmark_manifest_path=manifests_dir / "benchmark_release.sh",
            dataset_manifest_path=manifests_dir / "datasets.sh",
        )


def test_build_summary_fails_fast_on_release_mismatch_and_carries_release_fields(
    tmp_path,
):
    manifests_dir = tmp_path / "benchmarks/manifests"
    manifests_dir.mkdir(parents=True)
    (manifests_dir / "benchmark_release.sh").write_text(
        "\n".join(
            [
                'BENCHMARK_MANIFEST_VERSION="1"',
                'BENCHMARK_RELEASE="1.1"',
                'BENCHMARK_DATASET_RELEASE="1"',
                'BENCHMARK_LANGUAGES=("python")',
                'BENCHMARK_VARIANTS=("js-basic")',
                'BENCHMARK_SIZES=("10k")',
                'BENCHMARK_NOTES_REF=""',
                'BENCHMARK_STORAGE_PROVIDER="local"',
                'BENCHMARK_DEFAULT_DATASET_ROOT="benchmarks/datasets/prepared"',
                "",
            ]
        ),
        encoding="utf-8",
    )
    (manifests_dir / "datasets.sh").write_text(
        "\n".join(
            [
                'DATASET_MANIFEST_VERSION="1"',
                'DATASET_RELEASE="1"',
                'DATASET_STORAGE_PROVIDER="local"',
                'DATASET_BASE_URL=""',
                "declare -A DATASET_SHA256=()",
                "",
            ]
        ),
        encoding="utf-8",
    )
    metadata = load_release_metadata(
        benchmark_manifest_path=manifests_dir / "benchmark_release.sh",
        dataset_manifest_path=manifests_dir / "datasets.sh",
    )

    raw_payload = {
        "benchmark_release": "1.1",
        "dataset_release": 1,
        "language": "python",
        "environment": {"cpu_model": "x"},
        "issues": [],
        "results": [
            {
                "variant": "js-basic",
                "shape": "object",
                "size": "10k",
                "case": "json_serialize",
                "warmups": 1,
                "iterations": 2,
                "stats": {
                    "mean_ms": 1.0,
                    "median_ms": 1.0,
                    "min_ms": 1.0,
                    "max_ms": 1.0,
                    "stdev_ms": 0.0,
                },
                "samples_ms": [1.0, 1.0],
                "input_hint": "fixture",
            }
        ],
    }

    summary = build_summary([raw_payload], metadata)

    assert summary["benchmark_release"] == "1.1"
    assert summary["dataset_release"] == 1
    assert summary["results"][0]["benchmark_release"] == "1.1"
    assert summary["results"][0]["dataset_release"] == 1

    raw_payload["dataset_release"] = 9
    with pytest.raises(ValueError, match="dataset_release mismatch"):
        build_summary([raw_payload], metadata)


def test_ensure_prepared_datasets_downloads_missing_archive_and_unpacks(
    tmp_path, monkeypatch
):
    prepared_root = tmp_path / "prepared"
    archive_payload = b"fake-zstd-archive"
    archive_sha256 = hashlib.sha256(archive_payload).hexdigest()
    dataset_manifest = DatasetManifest(
        manifest_version=1,
        dataset_release=7,
        storage_provider="r2",
        base_url="https://example.com",
        sha256_by_key={"js-basic/10k": archive_sha256},
    )

    monkeypatch.setattr(
        benchmark_datasets,
        "load_release_metadata",
        lambda: SimpleNamespace(dataset_manifest=dataset_manifest),
    )

    class _FakeResponse(io.BytesIO):
        def __enter__(self):
            return self

        def __exit__(self, exc_type, exc, tb):
            self.close()
            return False

    requested_urls: list[str] = []

    def fake_urlopen(url: str):
        requested_urls.append(url)
        return _FakeResponse(archive_payload)

    monkeypatch.setattr(benchmark_datasets, "urlopen", fake_urlopen)

    import benchmarks.python.common.dataset_generation as dataset_generation

    def fake_unpack_archive(
        archive_path: Path, destination_parent: Path, *, overwrite: bool = False
    ) -> Path:
        assert overwrite is False
        bundle_root = destination_parent / "10k"
        bundle_root.mkdir(parents=True, exist_ok=True)
        (bundle_root / "meta.json").write_text(
            '{"schema_version":1,"variant":"js-basic","size":"10k","row_count":10000,'
            '"schema_summary":{},"source_provenance":{},"generator_version":"test",'
            '"prepared_at":"2026-04-01T00:00:00+00:00","available_benchmark_targets":{}}',
            encoding="utf-8",
        )
        return bundle_root

    monkeypatch.setattr(dataset_generation, "unpack_archive", fake_unpack_archive)

    datasets, unpacked = benchmark_datasets.ensure_prepared_datasets(
        prepared_root,
        variant="js-basic",
        size="10k",
    )

    archive_path = prepared_root / "js-basic" / "10k.tar.zst"
    assert requested_urls == [
        "https://example.com/datasets/releases/7/js-basic/10k.tar.zst"
    ]
    assert archive_path.read_bytes() == archive_payload
    assert unpacked == [prepared_root / "js-basic" / "10k"]
    assert len(datasets) == 1
    assert datasets[0].variant == "js-basic"
    assert datasets[0].size == "10k"
