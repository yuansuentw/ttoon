"""
Fixture-driven tests for Python.
Loads shared test cases from tests/fixtures/*.json.
"""
from __future__ import annotations

import io
import json
import math
from datetime import date, time, datetime
from decimal import Decimal
from pathlib import Path
from typing import Any
import uuid

import pytest  # type: ignore[import-not-found]
import ttoon

FIXTURES_DIR = Path(__file__).resolve().parent.parent.parent / "tests" / "fixtures"


def _skip_if_core_missing():
    if ttoon._core is None:
        pytest.skip("core extension not available")


def load_fixture(name: str) -> dict[str, Any]:
    path = FIXTURES_DIR / name
    with open(path) as f:
        return json.load(f)


def should_skip(test: dict[str, Any], fixture: dict[str, Any] | None = None) -> bool:
    if "python" in test.get("skip", []):
        return True
    if fixture and "python" in fixture.get("skip_default", []):
        return True
    return False


def parse_mode_kwargs(test: dict[str, Any]) -> dict[str, Any]:
    mode = test.get("mode")
    return {"mode": mode} if mode is not None else {}


def native_from_fixture(obj: dict[str, Any]) -> Any:
    """Convert type-tagged JSON value to Python native value."""
    t = obj["type"]

    if t == "null":
        return None
    if t == "bool":
        return obj["value"]
    if t == "int":
        v = obj["value"]
        return int(v) if isinstance(v, str) else v
    if t == "float":
        v = obj["value"]
        if isinstance(v, str):
            mapping = {
                "NaN": float("nan"),
                "+Infinity": float("inf"),
                "-Infinity": float("-inf"),
                "-0.0": -0.0,
            }
            if v in mapping:
                return mapping[v]
            raise ValueError(f"Unknown float special: {v}")
        return float(v)
    if t == "decimal":
        s = obj["value"]
        # Strip trailing 'm' suffix
        return Decimal(s[:-1] if s.endswith("m") else s)
    if t == "string":
        return obj["value"]
    if t == "date":
        return date.fromisoformat(obj["value"])
    if t == "time":
        return time.fromisoformat(obj["value"])
    if t == "datetime":
        s = obj["value"]
        # Python's fromisoformat handles most ISO 8601 formats
        # but 'Z' needs to be replaced with '+00:00' in older Python
        s = s.replace("Z", "+00:00")
        return datetime.fromisoformat(s)
    if t == "uuid":
        return uuid.UUID(obj["value"])
    if t in ("binary_hex", "binary_b64"):
        if t == "binary_hex":
            v = obj["value"]
            return bytes.fromhex(v) if v else b""
        else:
            import base64
            v = obj["value"]
            return base64.b64decode(v) if v else b""
    if t == "list":
        return [native_from_fixture(item) for item in obj["value"]]
    if t == "object":
        return {k: native_from_fixture(v) for k, v in obj["value"].items()}

    raise ValueError(f"Unknown type: {t}")


def assert_equal_with_nan(actual: Any, expected: Any, context: str = "") -> None:
    """Compare values, handling NaN and -0.0 specially."""
    if isinstance(expected, float):
        if math.isnan(expected):
            assert isinstance(actual, float) and math.isnan(actual), \
                f"{context}: expected NaN, got {actual!r}"
            return
        if expected == 0.0 and math.copysign(1.0, expected) < 0:
            assert isinstance(actual, float) and actual == 0.0 and math.copysign(1.0, actual) < 0, \
                f"{context}: expected -0.0, got {actual!r}"
            return
    if isinstance(expected, list):
        assert isinstance(actual, list) and len(actual) == len(expected), \
            f"{context}: list length mismatch"
        for i, (a, e) in enumerate(zip(actual, expected)):
            assert_equal_with_nan(a, e, f"{context}[{i}]")
        return
    if isinstance(expected, dict):
        assert isinstance(actual, dict) and set(actual.keys()) == set(expected.keys()), \
            f"{context}: dict keys mismatch"
        for k in expected:
            assert_equal_with_nan(actual[k], expected[k], f"{context}.{k}")
        return
    assert actual == expected, f"{context}: {actual!r} != {expected!r}"


def assert_serialize_output(test: dict[str, Any], text: str) -> None:
    test_id = test["id"]

    if "expected_output" in test:
        assert text.rstrip() == test["expected_output"].rstrip(), \
            f'{test_id}: expected {test["expected_output"]!r}, got {text!r}'

    if "expected_output_contains" in test:
        for s in test["expected_output_contains"]:
            assert s in text, \
                f'{test_id}: expected {s!r} in output, got: {text!r}'

    if "expected_output_starts_with" in test:
        assert text.startswith(test["expected_output_starts_with"]), \
            f'{test_id}: expected to start with {test["expected_output_starts_with"]!r}, got: {text!r}'

    if "expected_output_not_contains" in test:
        for s in test["expected_output_not_contains"]:
            assert s not in text, \
                f'{test_id}: expected {s!r} NOT in output, got: {text!r}'


def _stream_field_type_from_fixture(cell: dict[str, Any] | None):
    if not cell or cell["type"] == "null":
        return None

    match cell["type"]:
        case "string":
            return ttoon.types.string
        case "int":
            return ttoon.types.int
        case "float":
            return ttoon.types.float
        case "bool":
            return ttoon.types.bool
        case "date":
            return ttoon.types.date
        case "time":
            return ttoon.types.time
        case "datetime":
            value = cell["value"]
            if (
                isinstance(value, str)
                and (value.endswith("Z") or "T" in value and ("+" in value[10:] or "-" in value[10:]))
            ):
                return ttoon.types.datetime
            return ttoon.types.datetime_naive
        case "uuid":
            return ttoon.types.uuid
        case "binary_hex" | "binary_b64":
            return ttoon.types.binary
        case "decimal":
            value = cell["value"]
            if not isinstance(value, str):
                raise AssertionError(f"decimal fixture value must be string: {value!r}")
            normalized = value[:-1] if value.endswith("m") else value
            signless = normalized.lstrip("+-")
            whole, dot, frac = signless.partition(".")
            digits = (whole + frac).lstrip("0")
            precision = len(digits) if digits else 1
            scale = len(frac) if dot else 0
            return ttoon.types.decimal(precision, scale)
        case other:
            raise AssertionError(f"unsupported streaming fixture type: {other!r}")


def _stream_schema_from_fixture(
    fields: list[str],
    rows: list[dict[str, Any]],
) -> ttoon.StreamSchema:
    schema_fields: dict[str, Any] = {}

    for field in fields:
        nullable = False
        field_type = None
        for row in rows:
            cell = row.get(field)
            if not cell:
                continue
            if cell["type"] == "null":
                nullable = True
                continue
            if field_type is None:
                field_type = _stream_field_type_from_fixture(cell)
        if field_type is None:
            field_type = ttoon.types.string
        schema_fields[field] = field_type.nullable() if nullable else field_type

    return ttoon.StreamSchema(schema_fields)


def _stream_row_from_fixture(
    fields: list[str],
    row: dict[str, Any],
) -> dict[str, Any]:
    converted: dict[str, Any] = {}
    for field in fields:
        cell = row.get(field)
        if not cell or cell["type"] == "null":
            converted[field] = None
        else:
            converted[field] = native_from_fixture(cell)
    return converted


# ─── Helpers ───────────────────────────────────────────────────────────────────

def _collect_tests(fixture_name: str):
    """Collect non-skipped tests from a fixture file."""
    fixture = load_fixture(fixture_name)
    tests = []
    for test in fixture["tests"]:
        if should_skip(test, fixture):
            continue
        tests.append(test)
    return tests


def _test_ids(tests: list[dict[str, Any]]) -> list[str]:
    return [t["id"] for t in tests]


# ─── Format detection ─────────────────────────────────────────────────────────

class TestFormatDetect:
    tests = _collect_tests("format_detect.json")

    @pytest.mark.parametrize("test", tests, ids=_test_ids(tests))
    def test_format_detect(self, test: dict[str, Any]):
        _skip_if_core_missing()
        result = ttoon.detect_format(test["input"])
        expected = test["expected_format"]
        # v003 format names: "tjson", "ttoon", "typed_unit"
        assert result == expected, \
            f"format mismatch: got {result!r}, expected {expected!r} for input {test['input']!r}"
        if test.get("expected_parse_error"):
            with pytest.raises((ValueError, RuntimeError)) as excinfo:
                ttoon.loads(test["input"])
            needle = test.get("expected_parse_error_contains")
            if needle:
                assert needle in str(excinfo.value)


# ─── Parse tests ──────────────────────────────────────────────────────────────

def _run_parse_fixture(fixture_name: str):
    """Generate parse test cases from a fixture file."""
    tests = _collect_tests(fixture_name)

    @pytest.mark.parametrize("test", tests, ids=_test_ids(tests))
    def test_parse(self, test: dict[str, Any]):
        _skip_if_core_missing()
        if test.get("input") is None:
            pytest.skip("no input")
        parse_kwargs = parse_mode_kwargs(test)

        if test.get("expected") and "error" in test["expected"]:
            with pytest.raises((ValueError, RuntimeError)):
                ttoon.loads(test["input"], **parse_kwargs)
        elif test.get("expected"):
            result = ttoon.loads(test["input"], **parse_kwargs)
            expected_native = native_from_fixture(test["expected"])
            assert_equal_with_nan(result, expected_native, test["id"])
        # else: no expected value, just check no crash

    return test_parse


class TestParseScalars:
    test_parse = _run_parse_fixture("parse_scalars.json")


class TestParseIntegers:
    test_parse = _run_parse_fixture("parse_integers.json")


class TestParseFloats:
    test_parse = _run_parse_fixture("parse_floats.json")


class TestParseStrings:
    test_parse = _run_parse_fixture("parse_strings.json")


class TestParseTypedCells:
    test_parse = _run_parse_fixture("parse_typed_cells.json")


class TestParseDateTime:
    test_parse = _run_parse_fixture("parse_date_time.json")


class TestParseStructures:
    test_parse = _run_parse_fixture("parse_structures.json")


class TestParseTtoonStructure:
    test_parse = _run_parse_fixture("parse_ttoon_structure.json")


class TestParseTtoonTabularExact:
    test_parse = _run_parse_fixture("parse_ttoon_tabular_exact.json")


class TestParseTtoonTabularStreaming:
    test_parse = _run_parse_fixture("parse_ttoon_tabular_streaming.json")


class TestErrorLex:
    test_parse = _run_parse_fixture("error_lex.json")


# ─── Validation errors ────────────────────────────────────────────────────────

class TestValidationErrors:
    tests = _collect_tests("validation_errors.json")

    @pytest.mark.parametrize("test", tests, ids=_test_ids(tests))
    def test_validation(self, test: dict[str, Any]):
        _skip_if_core_missing()
        inp = test["input"]
        parse_kwargs = parse_mode_kwargs(test)
        with pytest.raises((ValueError, RuntimeError)):
            ttoon.loads(inp, **parse_kwargs)


# ─── Roundtrip tests ──────────────────────────────────────────────────────────

class TestRoundtrip:
    tests = _collect_tests("roundtrip.json")

    @pytest.mark.parametrize("test", tests, ids=_test_ids(tests))
    def test_roundtrip(self, test: dict[str, Any]):
        _skip_if_core_missing()
        value_obj = test.get("value")
        if value_obj is None:
            pytest.skip("no value")

        opts = test.get("options") or {}
        fmt = test.get("format", "tjson")
        kwargs: dict[str, Any] = {}
        if "delimiter" in opts:
            kwargs["delimiter"] = opts["delimiter"]
        if "indent_size" in opts:
            kwargs["indent_size"] = opts["indent_size"]
        if "binary_format" in opts:
            kwargs["binary_format"] = opts["binary_format"]

        native_val = native_from_fixture(value_obj)
        text = ttoon.dumps(native_val, **kwargs) if fmt == "ttoon" else ttoon.to_tjson(native_val, **kwargs)
        restored = ttoon.loads(text)
        assert_equal_with_nan(restored, native_val, test["id"])


def assert_arrow_field_matches(field: Any, spec: Any, context: str) -> None:
    pyarrow = pytest.importorskip("pyarrow")
    unit_map = {
        "second": "s",
        "millisecond": "ms",
        "microsecond": "us",
        "nanosecond": "ns",
    }

    if isinstance(spec, str):
        schema_shorthand_map: dict[str, dict[str, Any]] = {
            "int": {"type": "int64"},
            "int_nullable": {"type": "int64", "nullable": True},
            "float": {"type": "float64"},
            "bool": {"type": "bool"},
            "string": {"type": "utf8"},
        }
        spec = schema_shorthand_map[spec]

    ty = spec["type"]
    nullable = spec.get("nullable")

    expected_type: Any
    if ty == "null":
        expected_type = pyarrow.null()
    elif ty == "bool":
        expected_type = pyarrow.bool_()
    elif ty == "int64":
        expected_type = pyarrow.int64()
    elif ty == "float64":
        expected_type = pyarrow.float64()
    elif ty == "utf8":
        expected_type = pyarrow.string()
    elif ty == "binary":
        expected_type = pyarrow.binary()
    elif ty == "uuid":
        expected_type = pyarrow.uuid()
    elif ty == "date32":
        expected_type = pyarrow.date32()
    elif ty == "time64":
        expected_type = pyarrow.time64(unit_map.get(spec.get("unit", "microsecond"), "us"))
    elif ty == "timestamp":
        expected_type = pyarrow.timestamp(
            unit_map.get(spec.get("unit", "microsecond"), "us"),
            tz=spec.get("timezone"),
        )
    elif ty == "decimal128":
        expected_type = pyarrow.decimal128(spec["precision"], spec["scale"])
    elif ty == "decimal256":
        expected_type = pyarrow.decimal256(spec["precision"], spec["scale"])
    else:
        raise AssertionError(f"{context}: unknown schema type {ty!r}")

    assert field.type == expected_type, f"{context}: type mismatch: {field.type!r} != {expected_type!r}"
    if ty == "uuid":
        assert field.type.extension_name == "arrow.uuid", \
            f"{context}: uuid extension mismatch: {field.type!r}"
    if nullable is not None:
        assert field.nullable == nullable, f"{context}: nullable mismatch"


class TestReadArrow:
    tests = _collect_tests("read_arrow.json")

    @pytest.mark.parametrize("test", tests, ids=_test_ids(tests))
    def test_read_arrow(self, test: dict[str, Any]):
        _skip_if_core_missing()

        expected = test.get("expected")
        if expected and "error" in expected:
            with pytest.raises((ValueError, RuntimeError)):
                ttoon.read_arrow(test["input"])
            return

        table = ttoon.read_arrow(test["input"])

        if "expected_num_rows" in test:
            assert table.num_rows == test["expected_num_rows"]
        if "expected_num_cols" in test:
            assert table.num_columns == test["expected_num_cols"]
        if "expected_field_order" in test:
            assert table.schema.names == test["expected_field_order"]
        if "expected_schema" in test:
            for field_name, spec in test["expected_schema"].items():
                field = table.schema.field(field_name)
                assert_arrow_field_matches(field, spec, f"{test['id']}.{field_name}")
        if "expected_rows" in test:
            expected_rows = [native_from_fixture(row) for row in test["expected_rows"]]
            assert_equal_with_nan(table.to_pylist(), expected_rows, f"{test['id']}.rows")


# ─── Serialize options ─────────────────────────────────────────────────────────

class TestSerializeOptions:
    tests = _collect_tests("serialize_options.json")

    @pytest.mark.parametrize("test", tests, ids=_test_ids(tests))
    def test_serialize(self, test: dict[str, Any]):
        _skip_if_core_missing()
        value_obj = test.get("value")
        if value_obj is None:
            pytest.skip("no value")

        opts = test.get("options") or {}
        fmt = opts.get("format", "tjson")
        kwargs: dict[str, Any] = {}
        if "delimiter" in opts:
            kwargs["delimiter"] = opts["delimiter"]
        if "indent_size" in opts:
            kwargs["indent_size"] = opts["indent_size"]
        if "binary_format" in opts:
            kwargs["binary_format"] = opts["binary_format"]

        native_val = native_from_fixture(value_obj)

        # Error cases
        if test.get("expected") and "error" in test["expected"]:
            serialize_fn = ttoon.dumps if fmt == "ttoon" else ttoon.to_tjson
            with pytest.raises((ValueError, RuntimeError, TypeError)):
                serialize_fn(native_val, **kwargs)
            return

        # Serialize
        if fmt == "ttoon":
            text = ttoon.dumps(native_val, **kwargs)
        else:
            text = ttoon.to_tjson(native_val, **kwargs)

        assert isinstance(text, str)
        assert len(text) > 0
        assert_serialize_output(test, text)


class TestSerializeTtoonTabularExact:
    tests = _collect_tests("serialize_ttoon_tabular_exact.json")

    @pytest.mark.parametrize("test", tests, ids=_test_ids(tests))
    def test_serialize(self, test: dict[str, Any]):
        _skip_if_core_missing()
        value_obj = test.get("value")
        if value_obj is None:
            pytest.skip("no value")

        opts = test.get("options") or {}
        kwargs: dict[str, Any] = {}
        if "delimiter" in opts:
            kwargs["delimiter"] = opts["delimiter"]

        native_val = native_from_fixture(value_obj)
        text = ttoon.dumps(native_val, **kwargs)

        assert isinstance(text, str)
        assert len(text) > 0
        assert_serialize_output(test, text)

class TestSerializeTtoonTabularStreaming:
    tests = _collect_tests("serialize_ttoon_tabular_streaming.json")

    @pytest.mark.parametrize("test", tests, ids=_test_ids(tests))
    def test_serialize(self, test: dict[str, Any]):
        _skip_if_core_missing()

        fields = test["fields"]
        rows = test["rows"]
        schema = _stream_schema_from_fixture(fields, rows)
        sink = io.StringIO()

        with ttoon.stream_writer(sink, schema=schema) as writer:
            for row in rows:
                writer.write(_stream_row_from_fixture(fields, row))

        text = sink.getvalue()

        assert isinstance(text, str)
        assert_serialize_output(test, text)
