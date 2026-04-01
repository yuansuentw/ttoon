from __future__ import annotations

from dataclasses import dataclass
from datetime import date, datetime, time, timedelta, timezone
from decimal import Decimal
from pathlib import Path
from typing import Any
import json
import re
import shutil
import subprocess
import tarfile
import uuid

from benchmarks.python.common.native_source import encode_native_value

SIZE_TO_ROW_COUNT = {
    "10k": 10_000,
    "100k": 100_000,
    "1m": 1_000_000,
}

GENERATOR_VERSION = "benchmark-datasets-v1"
JS_BASIC_SEED_FILE = "js-basic.seed.json"
EXTENDED_NAMESPACE = uuid.UUID("f21a4cb7-fd3d-4fca-93d7-2caea78f5c15")
SAFE_BARE_STRING_RE = re.compile(r"^[A-Za-z0-9._/+:-]+$")


@dataclass(frozen=True)
class BundlePaths:
    variant: str
    size: str
    root: Path
    structure_dir: Path
    tabular_dir: Path
    archive_path: Path


def build_bundle_paths(prepared_root: Path, variant: str, size: str) -> BundlePaths:
    root = prepared_root / variant / size
    return BundlePaths(
        variant=variant,
        size=size,
        root=root,
        structure_dir=root / "structure",
        tabular_dir=root / "tabular",
        archive_path=prepared_root / variant / f"{size}.tar.zst",
    )


def generate_bundle(
    *,
    variant: str,
    size: str,
    source_root: Path,
    prepared_root: Path,
    prepared_at: str,
    overwrite: bool = False,
) -> dict[str, Any]:
    if size not in SIZE_TO_ROW_COUNT:
        raise ValueError(f"unsupported size: {size}")
    if variant not in {"js-basic", "extended"}:
        raise ValueError(f"unsupported variant: {variant}")

    paths = build_bundle_paths(prepared_root, variant, size)
    if paths.root.exists():
        if not overwrite:
            raise FileExistsError(f"bundle already exists: {paths.root}")
        shutil.rmtree(paths.root)

    paths.structure_dir.mkdir(parents=True, exist_ok=True)
    paths.tabular_dir.mkdir(parents=True, exist_ok=True)

    row_count = SIZE_TO_ROW_COUNT[size]
    if variant == "js-basic":
        meta = _generate_js_basic_bundle(paths, row_count, source_root, prepared_at)
    else:
        meta = _generate_extended_bundle(paths, row_count, prepared_at)

    meta_path = paths.root / "meta.json"
    meta_path.write_text(f"{json.dumps(meta, ensure_ascii=False, indent=2)}\n", encoding="utf-8")
    return meta


def create_archive(bundle_dir: Path, archive_path: Path) -> None:
    archive_path.parent.mkdir(parents=True, exist_ok=True)
    temporary_tar = archive_path.with_suffix("")
    if temporary_tar.suffix != ".tar":
        temporary_tar = temporary_tar.with_suffix(".tar")

    if temporary_tar.exists():
        temporary_tar.unlink()
    if archive_path.exists():
        archive_path.unlink()

    with tarfile.open(temporary_tar, "w") as tar:
        for path in _iter_bundle_members(bundle_dir):
            arcname = path.relative_to(bundle_dir.parent)
            info = tar.gettarinfo(str(path), arcname=str(arcname))
            info.uid = 0
            info.gid = 0
            info.uname = "root"
            info.gname = "root"
            info.mtime = 0
            if path.is_dir():
                info.mode = 0o755
                tar.addfile(info)
                continue

            info.mode = 0o644
            with path.open("rb") as handle:
                tar.addfile(info, handle)

    completed = subprocess.run(
        ["zstd", "-q", "-f", "-T1", str(temporary_tar)],
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        temporary_tar.unlink(missing_ok=True)
        raise RuntimeError(completed.stderr.strip() or "failed to create zstd archive")

    produced_archive = temporary_tar.with_suffix(".tar.zst")
    produced_archive.replace(archive_path)
    temporary_tar.unlink(missing_ok=True)


def unpack_archive(archive_path: Path, destination_parent: Path, *, overwrite: bool = False) -> Path:
    if not archive_path.is_file():
        raise FileNotFoundError(archive_path)

    bundle_name = archive_path.name.removesuffix(".tar.zst")
    destination = destination_parent / bundle_name
    if destination.exists():
        if not overwrite:
            raise FileExistsError(f"bundle already exists: {destination}")
        shutil.rmtree(destination)

    temporary_tar = archive_path.with_suffix("")
    if temporary_tar.suffix != ".tar":
        temporary_tar = temporary_tar.with_suffix(".tar")
    temporary_tar.unlink(missing_ok=True)

    completed = subprocess.run(
        ["zstd", "-q", "-d", "-f", str(archive_path), "-o", str(temporary_tar)],
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        temporary_tar.unlink(missing_ok=True)
        raise RuntimeError(completed.stderr.strip() or "failed to decompress archive")

    destination_parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(temporary_tar, "r") as tar:
        tar.extractall(destination_parent)
    temporary_tar.unlink(missing_ok=True)
    return destination


def _generate_js_basic_bundle(
    paths: BundlePaths,
    row_count: int,
    source_root: Path,
    prepared_at: str,
) -> dict[str, Any]:
    import pyarrow as pa
    import ttoon

    seed_path = source_root / "authoritative" / JS_BASIC_SEED_FILE
    seed_payload = json.loads(seed_path.read_text(encoding="utf-8"))
    seed_rows = seed_payload["rows"]
    columns = _build_js_basic_columns(row_count, seed_rows)
    object_source = _build_js_basic_object_source(paths.size, row_count, columns, seed_path)
    table = pa.table(
        {
            "row_id": pa.array(columns["row_id"], type=pa.int64()),
            "name": pa.array(columns["name"], type=pa.string()),
            "region": pa.array(columns["region"], type=pa.string()),
            "status": pa.array(columns["status"], type=pa.string()),
            "cohort": pa.array(columns["cohort"], type=pa.string()),
            "active": pa.array(columns["active"], type=pa.bool_()),
            "visits": pa.array(columns["visits"], type=pa.int64()),
            "score": pa.array(columns["score"], type=pa.float64()),
            "ratio": pa.array(columns["ratio"], type=pa.float64()),
            "note": pa.array(columns["note"], type=pa.string()),
        }
    )

    _write_text(paths.structure_dir / "source.json", json.dumps(object_source, ensure_ascii=False, indent=2))
    _write_text(paths.structure_dir / "tjson.txt", ttoon.to_tjson(object_source))
    ttoon_text = ttoon.dumps(object_source)
    _write_text(paths.structure_dir / "ttoon.txt", ttoon_text)
    _write_text(paths.structure_dir / "toon.txt", serialize_toon_subset(object_source))

    _write_arrow_file(paths.tabular_dir / "source.arrow", table)
    _write_text(paths.tabular_dir / "tjson.txt", ttoon.stringify_arrow_tjson(table))
    _write_text(paths.tabular_dir / "ttoon.txt", ttoon.dumps(table))

    return {
        "schema_version": 1,
        "variant": "js-basic",
        "size": paths.size,
        "row_count": row_count,
        "schema_summary": {
            "shape": "structure+tabular",
            "object_layout": "column-oriented-nested-object-v1",
            "object_source_encoding": "plain-json-v1",
            "fields": [
                {"name": "row_id", "kind": "int64", "nullable": False},
                {"name": "name", "kind": "string", "nullable": False},
                {"name": "region", "kind": "string", "nullable": False},
                {"name": "status", "kind": "string", "nullable": False},
                {"name": "cohort", "kind": "string", "nullable": False},
                {"name": "active", "kind": "bool", "nullable": False},
                {"name": "visits", "kind": "int64", "nullable": False},
                {"name": "score", "kind": "float64", "nullable": False},
                {"name": "ratio", "kind": "float64", "nullable": False},
                {"name": "note", "kind": "string", "nullable": True},
            ],
            "object_root_keys": ["meta", "profiles", "metrics", "flags", "notes"],
            "table_field_order": [
                "row_id",
                "name",
                "region",
                "status",
                "cohort",
                "active",
                "visits",
                "score",
                "ratio",
                "note",
            ],
        },
        "source_provenance": {
            "strategy": "authoritative",
            "details": f"normalized seed corpus from {seed_path.relative_to(source_root.parent.parent.parent)}",
        },
        "generator_version": GENERATOR_VERSION,
        "prepared_at": prepared_at,
        "available_benchmark_targets": {
            "structure": {
                "source_json": True,
                "tjson": True,
                "ttoon": True,
                "toon": True,
            },
            "tabular": {
                "source_arrow": True,
                "tjson": True,
                "ttoon": True,
            },
        },
        "expansion": {
            "seed_row_count": len(seed_rows),
            "rules": [
                "fixed seed row order with modulo selection",
                "safe integer fields increment by deterministic step",
                "float fields use deterministic offsets rounded to 6 decimals",
                "string fields append zero-padded suffixes",
                "nullable note field follows fixed stride pattern",
            ],
        },
    }


def _generate_extended_bundle(
    paths: BundlePaths,
    row_count: int,
    prepared_at: str,
) -> dict[str, Any]:
    import pyarrow as pa
    import ttoon

    columns = _build_extended_columns(row_count)
    object_source = _build_extended_object_source(paths.size, row_count, columns)
    encoded_object_source = encode_native_value(object_source)
    table = pa.table(
        {
            "row_id": pa.array(columns["row_id"], type=pa.int64()),
            "account_uuid": pa.array([value.bytes for value in columns["account_uuid"]], type=pa.uuid()),
            "amount": pa.array(columns["amount"], type=pa.decimal128(20, 4)),
            "trade_date": pa.array(columns["trade_date"], type=pa.date32()),
            "trade_time": pa.array(columns["trade_time"], type=pa.time64("us")),
            "event_at": pa.array(columns["event_at"], type=pa.timestamp("us", tz="UTC")),
            "desk": pa.array(columns["desk"], type=pa.string()),
            "active": pa.array(columns["active"], type=pa.bool_()),
            "note": pa.array(columns["note"], type=pa.string()),
        }
    )

    _write_text(paths.structure_dir / "source.json", json.dumps(encoded_object_source, ensure_ascii=False, indent=2))
    _write_text(paths.structure_dir / "tjson.txt", ttoon.to_tjson(object_source))
    _write_text(paths.structure_dir / "ttoon.txt", ttoon.dumps(object_source))

    _write_arrow_file(paths.tabular_dir / "source.arrow", table)
    _write_text(paths.tabular_dir / "tjson.txt", ttoon.stringify_arrow_tjson(table))
    _write_text(paths.tabular_dir / "ttoon.txt", ttoon.dumps(table))

    return {
        "schema_version": 1,
        "variant": "extended",
        "size": paths.size,
        "row_count": row_count,
        "schema_summary": {
            "shape": "structure+tabular",
            "object_layout": "column-oriented-nested-object-v1",
            "fields": [
                {"name": "row_id", "kind": "int64", "nullable": False},
                {"name": "account_uuid", "kind": "uuid", "nullable": False},
                {"name": "amount", "kind": "decimal128(20,4)", "nullable": False},
                {"name": "trade_date", "kind": "date32", "nullable": False},
                {"name": "trade_time", "kind": "time64[us]", "nullable": False},
                {"name": "event_at", "kind": "timestamp[us,UTC]", "nullable": False},
                {"name": "desk", "kind": "string", "nullable": False},
                {"name": "active", "kind": "bool", "nullable": False},
                {"name": "note", "kind": "string", "nullable": True},
            ],
            "typed_fields": ["account_uuid", "amount", "trade_date", "trade_time", "event_at"],
            "object_source_encoding": "typed-envelope-v1",
            "table_field_order": [
                "row_id",
                "account_uuid",
                "amount",
                "trade_date",
                "trade_time",
                "event_at",
                "desk",
                "active",
                "note",
            ],
        },
        "source_provenance": {
            "strategy": "generated",
            "details": "deterministic stdlib generator with fixed UUID namespace and fixed temporal origin",
        },
        "generator_version": GENERATOR_VERSION,
        "prepared_at": prepared_at,
        "available_benchmark_targets": {
            "structure": {
                "source_json": True,
                "tjson": True,
                "ttoon": True,
                "toon": False,
            },
            "tabular": {
                "source_arrow": True,
                "tjson": True,
                "ttoon": True,
            },
        },
        "expansion": {
            "uuid_namespace": str(EXTENDED_NAMESPACE),
            "rules": [
                "uuid uses uuid5(namespace, row-id)",
                "decimal uses fixed integer arithmetic with scale 4",
                "date/time/datetime use fixed origin plus deterministic steps",
                "nullable note field follows fixed stride pattern",
            ],
        },
    }


def _build_js_basic_columns(row_count: int, seed_rows: list[dict[str, Any]]) -> dict[str, list[Any]]:
    row_ids: list[int] = []
    names: list[str] = []
    regions: list[str] = []
    statuses: list[str] = []
    cohorts: list[str] = []
    actives: list[bool] = []
    visits: list[int] = []
    scores: list[float] = []
    ratios: list[float] = []
    notes: list[str | None] = []
    vip_flags: list[bool] = []

    seed_count = len(seed_rows)
    for index in range(row_count):
        seed = seed_rows[index % seed_count]
        cycle = index // seed_count
        row_number = index + 1

        row_ids.append(row_number)
        names.append(f"{seed['name']}-{row_number:07d}")
        regions.append(seed["region"])
        statuses.append(seed["status"])
        cohorts.append(f"{seed['region']}-{(cycle + seed['cohort_bias']) % 16:02d}")
        actives.append(bool((cycle + seed["active_bias"]) % 2))
        visits.append(seed["visits_base"] + cycle * seed["visits_step"] + (index % 13))
        score = seed["score_base"] + (cycle % 29) * 0.137 + (index % 7) * 0.013
        scores.append(round(score, 6))
        ratio = ((seed["ratio_base"] * 1000) + ((index * 17) % 211)) / 1000.0
        ratios.append(round(ratio, 6))
        if row_number % seed["note_null_stride"] == 0:
            notes.append(None)
        else:
            notes.append(f"{seed['note_prefix']}-{cycle:05d}-{index % 31:02d}")
        vip_flags.append(((index + seed["vip_bias"]) % 5) == 0)

    return {
        "row_id": row_ids,
        "name": names,
        "region": regions,
        "status": statuses,
        "cohort": cohorts,
        "active": actives,
        "visits": visits,
        "score": scores,
        "ratio": ratios,
        "note": notes,
        "vip": vip_flags,
    }


def _build_js_basic_object_source(
    size: str,
    row_count: int,
    columns: dict[str, list[Any]],
    seed_path: Path,
) -> dict[str, Any]:
    return {
        "meta": {
            "variant": "js-basic",
            "size": size,
            "row_count": row_count,
            "seed_snapshot": seed_path.name,
        },
        "profiles": {
            "row_id": columns["row_id"],
            "name": columns["name"],
            "region": columns["region"],
            "status": columns["status"],
            "cohort": columns["cohort"],
        },
        "metrics": {
            "visits": columns["visits"],
            "score": columns["score"],
            "ratio": columns["ratio"],
        },
        "flags": {
            "active": columns["active"],
            "vip": columns["vip"],
        },
        "notes": columns["note"],
    }


def _build_extended_columns(row_count: int) -> dict[str, list[Any]]:
    row_ids: list[int] = []
    account_uuids: list[uuid.UUID] = []
    amounts: list[Decimal] = []
    trade_dates: list[date] = []
    trade_times: list[time] = []
    event_ats: list[datetime] = []
    desks: list[str] = []
    actives: list[bool] = []
    notes: list[str | None] = []

    base_date = date(2026, 1, 1)
    base_datetime = datetime(2026, 1, 1, 9, 30, tzinfo=timezone.utc)
    desk_names = ("spot", "credit", "fx", "rates", "mm")

    for index in range(row_count):
        row_number = index + 1
        row_ids.append(row_number)
        account_uuids.append(uuid.uuid5(EXTENDED_NAMESPACE, f"account-{row_number:07d}"))

        sign = Decimal("-1") if index % 9 == 0 else Decimal("1")
        whole = Decimal(1000 + (index % 97) * 37 + index // 97)
        fractional = Decimal((index * 17) % 10_000) / Decimal("10000")
        amounts.append((sign * (whole + fractional)).quantize(Decimal("0.0001")))

        trade_dates.append(base_date + timedelta(days=index % 400))
        trade_times.append(
            time(
                hour=(9 + (index % 8)) % 24,
                minute=(index * 7) % 60,
                second=(index * 13) % 60,
                microsecond=(index * 104_729) % 1_000_000,
            )
        )
        event_ats.append(
            base_datetime
            + timedelta(
                seconds=index * 37,
                microseconds=(index * 104_729) % 1_000_000,
            )
        )
        desks.append(desk_names[index % len(desk_names)])
        actives.append(index % 2 == 0)
        notes.append(None if row_number % 11 == 0 else f"desk-{desk_names[index % len(desk_names)]}-{row_number:07d}")

    return {
        "row_id": row_ids,
        "account_uuid": account_uuids,
        "amount": amounts,
        "trade_date": trade_dates,
        "trade_time": trade_times,
        "event_at": event_ats,
        "desk": desks,
        "active": actives,
        "note": notes,
    }


def _build_extended_object_source(size: str, row_count: int, columns: dict[str, list[Any]]) -> dict[str, Any]:
    return {
        "meta": {
            "variant": "extended",
            "size": size,
            "row_count": row_count,
            "uuid_namespace": str(EXTENDED_NAMESPACE),
        },
        "ledger": {
            "row_id": columns["row_id"],
            "account_uuid": columns["account_uuid"],
            "amount": columns["amount"],
        },
        "schedule": {
            "trade_date": columns["trade_date"],
            "trade_time": columns["trade_time"],
            "event_at": columns["event_at"],
        },
        "attributes": {
            "desk": columns["desk"],
            "active": columns["active"],
        },
        "notes": columns["note"],
    }


def serialize_toon_subset(value: Any) -> str:
    lines = _render_toon_block(value, 0)
    return "\n".join(lines) + "\n"


def _render_toon_block(value: Any, level: int) -> list[str]:
    indent = "  " * level
    if not isinstance(value, dict):
        raise TypeError("TOON subset serializer expects top-level object")

    lines: list[str] = []
    for key, item in value.items():
        if _is_scalar(item):
            lines.append(f"{indent}{_format_key(key)}: {_format_scalar(item)}")
        elif isinstance(item, list):
            lines.append(f"{indent}{_format_key(key)}:")
            child_indent = "  " * (level + 1)
            for entry in item:
                if not _is_scalar(entry):
                    raise TypeError("TOON subset serializer only supports scalar arrays")
                lines.append(f"{child_indent}- {_format_scalar(entry)}")
        elif isinstance(item, dict):
            lines.append(f"{indent}{_format_key(key)}:")
            lines.extend(_render_toon_nested_object(item, level + 1))
        else:
            raise TypeError(f"unsupported TOON subset value: {type(item)!r}")
    return lines


def _render_toon_nested_object(value: dict[str, Any], level: int) -> list[str]:
    indent = "  " * level
    lines: list[str] = []
    for key, item in value.items():
        if _is_scalar(item):
            lines.append(f"{indent}{_format_key(key)}: {_format_scalar(item)}")
        elif isinstance(item, list):
            lines.append(f"{indent}{_format_key(key)}:")
            child_indent = "  " * (level + 1)
            for entry in item:
                if not _is_scalar(entry):
                    raise TypeError("TOON subset serializer only supports scalar arrays")
                lines.append(f"{child_indent}- {_format_scalar(entry)}")
        elif isinstance(item, dict):
            lines.append(f"{indent}{_format_key(key)}:")
            lines.extend(_render_toon_nested_object(item, level + 1))
        else:
            raise TypeError(f"unsupported TOON subset value: {type(item)!r}")
    return lines


def _is_scalar(value: Any) -> bool:
    return value is None or isinstance(value, (bool, int, float, str))


def _format_key(key: str) -> str:
    if SAFE_BARE_STRING_RE.fullmatch(key):
        return key
    return json.dumps(key, ensure_ascii=False)


def _format_scalar(value: Any) -> str:
    if value is None:
        return "null"
    if value is True:
        return "true"
    if value is False:
        return "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, float):
        text = repr(value)
        return text if text != "nan" else "null"
    if isinstance(value, str):
        if value and SAFE_BARE_STRING_RE.fullmatch(value) and value not in {"null", "true", "false"}:
            return value
        return json.dumps(value, ensure_ascii=False)
    raise TypeError(f"unsupported scalar: {type(value)!r}")


def _write_text(path: Path, content: str) -> None:
    path.write_text(content, encoding="utf-8")


def _write_arrow_file(path: Path, table: Any) -> None:
    import pyarrow.ipc as ipc

    with path.open("wb") as sink:
        with ipc.new_file(sink, table.schema) as writer:
            writer.write_table(table)


def _iter_bundle_members(bundle_dir: Path) -> list[Path]:
    return [bundle_dir, *sorted(bundle_dir.rglob("*"))]
