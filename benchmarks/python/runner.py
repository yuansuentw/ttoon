from __future__ import annotations

import argparse
import gc
import json
import resource
import sys
import time
from pathlib import Path
from typing import Any, Callable

ROOT = Path(__file__).resolve().parents[2]
PYTHON_ROOT = ROOT / "python"
for path in (ROOT, PYTHON_ROOT):
    path_str = str(path)
    if path_str not in sys.path:
        sys.path.insert(0, path_str)

from benchmarks.python.common.config import (  # noqa: E402
    CASE_MATRIX,
    DEFAULT_ITERATIONS,
    DEFAULT_WARMUPS,
    RUNNER_SCHEMA_VERSION,
)
from benchmarks.python.common.datasets import (
    discover_prepared_datasets,
    ensure_prepared_datasets,
)  # noqa: E402
from benchmarks.python.common.manifests import load_release_metadata  # noqa: E402
from benchmarks.python.common.metrics import measure_sync  # noqa: E402
from benchmarks.python.common.reporting import collect_host_environment, utc_now_iso  # noqa: E402
from benchmarks.python.loaders.prepared import (  # noqa: E402
    load_arrow_table,
    load_object_source,
    load_text,
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Python benchmark runner")
    parser.add_argument(
        "--dataset-root", type=Path, default=ROOT / "benchmarks/datasets/prepared"
    )
    parser.add_argument("--variant", choices=("js-basic", "extended"))
    parser.add_argument("--size", choices=("10k", "100k", "1m"))
    parser.add_argument("--shape", choices=("structure", "tabular"))
    parser.add_argument("--case")
    parser.add_argument("--warmups", type=int, default=DEFAULT_WARMUPS)
    parser.add_argument("--iterations", type=int, default=DEFAULT_ITERATIONS)
    parser.add_argument("--benchmark-release")
    parser.add_argument("--dataset-release", type=int)
    parser.add_argument("--list-cases", action="store_true")
    parser.add_argument("--no-auto-unpack", action="store_true")
    parser.add_argument("--trace-memory", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.list_cases:
        print(
            json.dumps(
                _list_case_entries(args.variant, args.shape),
                ensure_ascii=False,
                indent=2,
            )
        )
        return 0

    payload = run_benchmarks(args)
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


def run_benchmarks(args: argparse.Namespace) -> dict[str, Any]:
    release_metadata = _resolve_release_metadata(args)
    issues: list[str] = []
    results: list[dict[str, Any]] = []
    if args.no_auto_unpack:
        datasets = discover_prepared_datasets(
            args.dataset_root,
            variant=args.variant,
            size=args.size,
        )
    else:
        datasets, _ = ensure_prepared_datasets(
            args.dataset_root,
            variant=args.variant,
            size=args.size,
        )

    try:
        import ttoon  # type: ignore

        _ensure_release_ttoon_core(ttoon)
    except ImportError as exc:  # pragma: no cover - environment dependent
        ttoon = None
        issues.append(f"failed to import ttoon: {exc}")

    cache: dict[tuple[Path, str], Any] = {}

    for bundle in datasets:
        for shape in CASE_MATRIX["python"][bundle.variant]:
            if args.shape and shape != args.shape:
                continue
            for case_name in CASE_MATRIX["python"][bundle.variant][shape]:
                if args.case and case_name != args.case:
                    continue
                prepared = _prepare_case(bundle, shape, case_name, cache, ttoon, issues)
                if prepared is None:
                    continue
                func, samples_input = prepared
                if args.trace_memory:
                    samples_ms, stats, memory_trace_kb = _measure_sync_with_memory(
                        func,
                        warmups=args.warmups,
                        iterations=args.iterations,
                        disable_gc=True,
                    )
                else:
                    samples_ms, stats = measure_sync(
                        func,
                        warmups=args.warmups,
                        iterations=args.iterations,
                        disable_gc=True,
                    )
                    memory_trace_kb = None

                result = {
                    "variant": bundle.variant,
                    "shape": shape,
                    "size": bundle.size,
                    "row_count": bundle.row_count,
                    "case": case_name,
                    "warmups": args.warmups,
                    "iterations": args.iterations,
                    "stats": stats.to_dict(),
                    "samples_ms": samples_ms,
                    "input_hint": samples_input,
                }
                if memory_trace_kb is not None:
                    result["memory_trace_kb"] = memory_trace_kb
                results.append(result)

    return {
        "schema_version": RUNNER_SCHEMA_VERSION,
        "benchmark_release": release_metadata.benchmark_release,
        "dataset_release": release_metadata.dataset_release,
        "language": "python",
        "generated_at": utc_now_iso(),
        "filters": {
            "variant": args.variant,
            "size": args.size,
            "shape": args.shape,
            "case": args.case,
            "warmups": args.warmups,
            "iterations": args.iterations,
        },
        "environment": collect_host_environment(),
        "dataset_count": len(datasets),
        "results": results,
        "issues": issues,
    }


def _ensure_release_ttoon_core(ttoon_module: Any) -> None:
    core = getattr(ttoon_module, "_core", None)
    profile = getattr(core, "BUILD_PROFILE", None)
    if profile != "release":
        raise RuntimeError(
            "Python benchmark requires a release-built ttoon._core; "
            "run `cd python && uv run maturin develop --release` first"
        )


def _resolve_release_metadata(args: argparse.Namespace) -> Any:
    release_metadata = load_release_metadata()
    benchmark_release = getattr(args, "benchmark_release", None)
    dataset_release = getattr(args, "dataset_release", None)

    if (
        benchmark_release is not None
        and benchmark_release != release_metadata.benchmark_release
    ):
        raise ValueError(
            "CLI benchmark_release does not match authoritative manifest: "
            f"{benchmark_release} vs {release_metadata.benchmark_release}"
        )
    if (
        dataset_release is not None
        and dataset_release != release_metadata.dataset_release
    ):
        raise ValueError(
            "CLI dataset_release does not match authoritative manifest: "
            f"{dataset_release} vs {release_metadata.dataset_release}"
        )
    return release_metadata


def _prepare_case(
    bundle: Any,
    shape: str,
    case_name: str,
    cache: dict[tuple[Path, str], Any],
    ttoon: Any,
    issues: list[str],
) -> tuple[Callable[[], object], str] | None:
    if case_name.startswith("json_"):
        if shape != "structure":
            return None
        source_path = bundle.file_for("structure", "source.json")
        if not source_path.is_file():
            issues.append(
                f"{bundle.meta_path}: missing JSON structure source, skipping {case_name}"
            )
            return None
        if case_name == "json_serialize":
            value = _load_cached(cache, source_path, load_object_source)
            return lambda: json.dumps(value), _display_path(source_path)
        text = _load_cached(cache, source_path, load_text)
        return lambda: json.loads(text), _display_path(source_path)

    if ttoon is None or getattr(ttoon, "_core", None) is None:
        issues.append(
            f"{bundle.meta_path}: ttoon core unavailable, skipping {case_name}"
        )
        return None

    if shape == "structure":
        source_path = bundle.file_for("structure", "source.json")
        tjson_path = bundle.file_for("structure", "tjson.txt")
        ttoon_path = bundle.file_for("structure", "ttoon.txt")
        if case_name == "tjson_serialize":
            if not source_path.is_file():
                issues.append(
                    f"{bundle.meta_path}: missing structure/source.json, skipping {case_name}"
                )
                return None
            value = _load_cached(cache, source_path, load_object_source)
            return lambda: ttoon.to_tjson(value), _display_path(source_path)
        if case_name == "tjson_deserialize":
            if not tjson_path.is_file():
                issues.append(
                    f"{bundle.meta_path}: missing structure/tjson.txt, skipping {case_name}"
                )
                return None
            text = _load_cached(cache, tjson_path, load_text)
            return lambda: ttoon.loads(text), _display_path(tjson_path)
        if case_name == "ttoon_serialize":
            if not source_path.is_file():
                issues.append(
                    f"{bundle.meta_path}: missing structure/source.json, skipping {case_name}"
                )
                return None
            value = _load_cached(cache, source_path, load_object_source)
            return lambda: ttoon.dumps(value), _display_path(source_path)
        if case_name == "ttoon_deserialize":
            if not ttoon_path.is_file():
                issues.append(
                    f"{bundle.meta_path}: missing structure/ttoon.txt, skipping {case_name}"
                )
                return None
            text = _load_cached(cache, ttoon_path, load_text)
            return lambda: ttoon.loads(text), _display_path(ttoon_path)
        return None

    source_arrow_path = bundle.file_for("tabular", "source.arrow")
    table_tjson_path = bundle.file_for("tabular", "tjson.txt")
    table_ttoon_path = bundle.file_for("tabular", "ttoon.txt")

    if case_name in {
        "arrow_ttoon_stream_serialize",
        "arrow_ttoon_stream_deserialize",
        "arrow_tjson_stream_serialize",
        "arrow_tjson_stream_deserialize",
    }:
        if not source_arrow_path.is_file():
            issues.append(
                f"{bundle.meta_path}: missing tabular/source.arrow, skipping {case_name}"
            )
            return None
        table = _load_cached(cache, source_arrow_path, load_arrow_table)
        try:
            stream_schema = _stream_schema_from_pyarrow(table.schema)
        except Exception as exc:
            issues.append(
                f"{bundle.meta_path}: failed to build streaming schema, skipping {case_name}: {exc}"
            )
            return None

        if case_name == "arrow_ttoon_stream_serialize":
            return (
                lambda: _stream_write_arrow_table(ttoon, table, stream_schema),
                _display_path(source_arrow_path),
            )
        if case_name == "arrow_tjson_stream_serialize":
            return (
                lambda: _stream_write_arrow_tjson(ttoon, table, stream_schema),
                _display_path(source_arrow_path),
            )
        if case_name == "arrow_ttoon_stream_deserialize":
            if not table_ttoon_path.is_file():
                issues.append(
                    f"{bundle.meta_path}: missing tabular/ttoon.txt, skipping {case_name}"
                )
                return None
            return (
                lambda: _stream_read_arrow_table(
                    ttoon, table_ttoon_path, stream_schema
                ),
                _display_path(table_ttoon_path),
            )
        if case_name == "arrow_tjson_stream_deserialize":
            if not table_tjson_path.is_file():
                issues.append(
                    f"{bundle.meta_path}: missing tabular/tjson.txt, skipping {case_name}"
                )
                return None
            return (
                lambda: _stream_read_tjson(ttoon, table_tjson_path, stream_schema),
                _display_path(table_tjson_path),
            )

    if case_name == "arrow_tjson_serialize":
        if not source_arrow_path.is_file():
            issues.append(
                f"{bundle.meta_path}: missing tabular/source.arrow, skipping {case_name}"
            )
            return None
        table = _load_cached(cache, source_arrow_path, load_arrow_table)
        return lambda: ttoon.stringify_arrow_tjson(table), _display_path(
            source_arrow_path
        )
    if case_name == "arrow_tjson_deserialize":
        if not table_tjson_path.is_file():
            issues.append(
                f"{bundle.meta_path}: missing tabular/tjson.txt, skipping {case_name}"
            )
            return None
        text = _load_cached(cache, table_tjson_path, load_text)
        return lambda: ttoon.read_arrow(text), _display_path(table_tjson_path)
    if case_name == "arrow_ttoon_serialize":
        if not source_arrow_path.is_file():
            issues.append(
                f"{bundle.meta_path}: missing tabular/source.arrow, skipping {case_name}"
            )
            return None
        table = _load_cached(cache, source_arrow_path, load_arrow_table)
        return lambda: ttoon.dumps(table), _display_path(source_arrow_path)
    if case_name == "arrow_ttoon_deserialize":
        if not table_ttoon_path.is_file():
            issues.append(
                f"{bundle.meta_path}: missing tabular/ttoon.txt, skipping {case_name}"
            )
            return None
        text = _load_cached(cache, table_ttoon_path, load_text)
        return lambda: ttoon.read_arrow(text), _display_path(table_ttoon_path)
    return None


def _load_cached(
    cache: dict[tuple[Path, str], Any], path: Path, loader: Callable[[Path], Any]
) -> Any:
    key = (path, loader.__name__)
    if key not in cache:
        cache[key] = loader(path)
    return cache[key]


def _measure_sync_with_memory(
    func: Callable[[], object],
    *,
    warmups: int,
    iterations: int,
    disable_gc: bool,
) -> tuple[list[float], Any, list[int]]:
    if warmups < 0 or iterations <= 0:
        raise ValueError("warmups must be >= 0 and iterations must be > 0")

    gc_was_enabled = gc.isenabled()
    if disable_gc and gc_was_enabled:
        gc.disable()

    try:
        for _ in range(warmups):
            func()

        samples_ms: list[float] = []
        memory_trace_kb: list[int] = []
        for _ in range(iterations):
            start_ns = time.perf_counter_ns()
            func()
            elapsed_ns = time.perf_counter_ns() - start_ns
            samples_ms.append(elapsed_ns / 1_000_000)
            memory_trace_kb.append(_current_rss_kb())
    finally:
        if disable_gc and gc_was_enabled:
            gc.enable()

    from benchmarks.python.common.metrics import summarize_samples

    return samples_ms, summarize_samples(samples_ms), memory_trace_kb


def _current_rss_kb() -> int:
    rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    if sys.platform == "darwin":
        return int(rss / 1024)
    return int(rss)


def _stream_schema_from_pyarrow(schema: Any) -> Any:
    import pyarrow as pa
    import ttoon  # type: ignore

    fields: dict[str, Any] = {}
    for field in schema:
        field_type = field.type
        extension_name = getattr(field_type, "extension_name", None)
        storage_type = getattr(field_type, "storage_type", None)
        if extension_name == "arrow.uuid":
            spec = ttoon.types.uuid
        if pa.types.is_string(field_type) or pa.types.is_large_string(field_type):
            spec = ttoon.types.string
        elif pa.types.is_int64(field_type):
            spec = ttoon.types.int
        elif pa.types.is_float64(field_type):
            spec = ttoon.types.float
        elif pa.types.is_boolean(field_type):
            spec = ttoon.types.bool
        elif pa.types.is_date32(field_type):
            spec = ttoon.types.date
        elif pa.types.is_time64(field_type):
            if getattr(field_type, "unit", None) != "us":
                raise ValueError(f"unsupported Time64 unit: {field_type}")
            spec = ttoon.types.time
        elif pa.types.is_timestamp(field_type):
            if getattr(field_type, "unit", None) != "us":
                raise ValueError(f"unsupported Timestamp unit: {field_type}")
            spec = ttoon.types.datetime if field_type.tz else ttoon.types.datetime_naive
        elif pa.types.is_decimal(field_type):
            spec = ttoon.types.decimal(field_type.precision, field_type.scale)
        elif pa.types.is_binary(field_type) or pa.types.is_large_binary(field_type):
            spec = ttoon.types.binary
        elif pa.types.is_fixed_size_binary(field_type):
            metadata = field.metadata or {}
            extension_name = metadata.get(b"ARROW:extension:name")
            spec = (
                ttoon.types.uuid
                if field_type.byte_width == 16 and extension_name == b"arrow.uuid"
                else ttoon.types.binary
            )
        elif pa.types.is_fixed_size_binary(storage_type):
            spec = (
                ttoon.types.uuid
                if getattr(storage_type, "byte_width", None) == 16
                and extension_name == "arrow.uuid"
                else ttoon.types.binary
            )
        else:
            raise ValueError(
                f"unsupported Arrow field type for streaming benchmark: {field_type}"
            )

        if field.nullable:
            spec = spec.nullable()
        fields[field.name] = spec

    return ttoon.StreamSchema(fields)


class _DiscardTextSink:
    def write(self, text: str) -> int:
        return len(text)

    def flush(self) -> None:
        return None


def _stream_write_arrow_table(
    ttoon_module: Any,
    table: Any,
    schema: Any,
) -> None:
    sink = _DiscardTextSink()
    with ttoon_module.stream_writer_arrow(sink, schema=schema) as writer:
        for batch in table.to_batches():
            writer.write_batch(batch)


def _stream_write_arrow_tjson(
    ttoon_module: Any,
    table: Any,
    schema: Any,
) -> None:
    sink = _DiscardTextSink()
    with ttoon_module.stream_writer_arrow_tjson(sink, schema=schema) as writer:
        for batch in table.to_batches():
            writer.write_batch(batch)


def _stream_read_tjson(
    ttoon_module: Any,
    text_path: Path,
    schema: Any,
) -> int:
    row_count = 0
    with text_path.open("r", encoding="utf-8", newline="") as source:
        for batch in ttoon_module.stream_read_arrow_tjson(
            source, schema=schema, batch_size=1024
        ):
            row_count += batch.num_rows
    return row_count


def _stream_read_arrow_table(
    ttoon_module: Any,
    text_path: Path,
    schema: Any,
) -> int:
    row_count = 0
    with text_path.open("r", encoding="utf-8", newline="") as source:
        for batch in ttoon_module.stream_read_arrow(
            source, schema=schema, batch_size=1024
        ):
            row_count += batch.num_rows
    return row_count


def _list_case_entries(
    variant: str | None,
    shape: str | None,
) -> list[dict[str, str]]:
    results: list[dict[str, str]] = []
    variants = (variant,) if variant else CASE_MATRIX["python"].keys()
    for variant_name in variants:
        shapes = (shape,) if shape else CASE_MATRIX["python"][variant_name].keys()
        for shape_name in shapes:
            for case_name in CASE_MATRIX["python"][variant_name][shape_name]:
                results.append(
                    {
                        "language": "python",
                        "variant": variant_name,
                        "shape": shape_name,
                        "case": case_name,
                    }
                )
    return results


def _display_path(path: Path) -> str:
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


if __name__ == "__main__":
    raise SystemExit(main())
