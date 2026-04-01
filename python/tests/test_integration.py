# Shared test cases (roundtrip, parsing, format detection, etc.) are in tests/fixtures/
# This file contains only integration tests unique to the Python package.

from typing import Any

import pytest  # type: ignore[import-not-found]
import ttoon


def _skip_if_core_missing():
    if ttoon._core is None:
        pytest.skip("core extension not available")


def test_polars_dataframe_roundtrip_numeric_types():
    _skip_if_core_missing()
    polars = pytest.importorskip("polars")
    df = polars.DataFrame(
        {
            "int_col": [1, -2, 3],
            "float_col": [1.5, -2.25, 0.0],
            "bool_col": [True, False, True],
        }
    )
    text = ttoon.dumps(df)
    restored: Any = polars.from_arrow(ttoon.read_arrow(text))

    assert isinstance(restored, polars.DataFrame)
    assert restored.schema == {
        "int_col": polars.Int64,
        "float_col": polars.Float64,
        "bool_col": polars.Boolean,
    }
    assert restored["int_col"].to_list() == [1, -2, 3]
    assert restored["float_col"].to_list() == [1.5, -2.25, 0.0]
    assert restored["bool_col"].to_list() == [True, False, True]


def test_arrow_table_roundtrip_numeric_types():
    _skip_if_core_missing()
    pyarrow = pytest.importorskip("pyarrow")
    table = pyarrow.table(
        {
            "int_col": pyarrow.array([1, -2, 3], type=pyarrow.int64()),
            "float_col": pyarrow.array([1.5, -2.25, 0.0], type=pyarrow.float64()),
            "bool_col": pyarrow.array([True, False, True], type=pyarrow.bool_()),
        }
    )
    text = ttoon.dumps(table)
    restored: Any = ttoon.read_arrow(text)

    assert isinstance(restored, pyarrow.Table)
    assert restored.schema.types == table.schema.types
    assert restored.column("int_col").to_pylist() == [1, -2, 3]
    assert restored.column("float_col").to_pylist() == [1.5, -2.25, 0.0]
    assert restored.column("bool_col").to_pylist() == [True, False, True]


def test_datetime_without_timezone_roundtrip():
    """Round-trip a timezone-naive datetime value."""
    _skip_if_core_missing()
    from datetime import datetime

    datetimes = [
        datetime(2026, 1, 20, 14, 30, 0),
        datetime(2026, 12, 31, 23, 59, 59),
        datetime(2026, 1, 20, 14, 30, 0, 125000),  # Includes microseconds
        datetime(1970, 1, 1, 0, 0, 0),
    ]

    for dt in datetimes:
        text = ttoon.dumps(dt)
        restored = ttoon.loads(text)
        assert restored == dt
        assert isinstance(restored, datetime)
        assert restored.tzinfo is None


def test_temporal_types_render_expected_text():
    _skip_if_core_missing()
    from datetime import date, datetime, time, timezone, timedelta

    assert ttoon.dumps(date(2026, 1, 20)).rstrip() == "2026-01-20"
    assert ttoon.dumps(time(14, 30, 0, 125000)).rstrip() == "14:30:00.125000"
    assert ttoon.dumps(datetime(2026, 1, 20, 14, 30, 0, 125000)).rstrip() == "2026-01-20T14:30:00.125000"
    assert (
        ttoon.dumps(
            datetime(
                2026,
                1,
                20,
                14,
                30,
                0,
                125000,
                tzinfo=timezone(timedelta(hours=8)),
            )
        ).rstrip()
        == "2026-01-20T14:30:00.125000+08:00"
    )


def test_datetime_with_timezone_roundtrip():
    _skip_if_core_missing()
    from datetime import datetime, timezone, timedelta

    datetimes = [
        datetime(2026, 1, 20, 14, 30, 0, tzinfo=timezone.utc),
        datetime(2026, 1, 20, 14, 30, 0, 125000, tzinfo=timezone.utc),
        datetime(2026, 1, 20, 14, 30, 0, tzinfo=timezone(timedelta(hours=8))),
        datetime(2026, 1, 20, 14, 30, 0, 125000, tzinfo=timezone(timedelta(hours=-5, minutes=-30))),
    ]

    for dt in datetimes:
        text = ttoon.dumps(dt)
        restored = ttoon.loads(text)
        assert restored == dt
        assert isinstance(restored, datetime)
        assert restored.tzinfo == dt.tzinfo


def test_decimal_type_roundtrip():
    """Round-trip Decimal values."""
    _skip_if_core_missing()
    from decimal import Decimal

    decimals = [
        Decimal("123.45"),
        Decimal("0"),
        Decimal("0.0"),
        Decimal("999999.999999"),
        Decimal("-123.45"),
        Decimal("100"),
        Decimal("0.123456789012345678901234567890"),
    ]

    for dec in decimals:
        text = ttoon.dumps(dec)
        restored = ttoon.loads(text)
        assert restored == dec
        assert isinstance(restored, Decimal)


def test_binary_hex_format_roundtrip():
    """Round-trip binary values through the hex format."""
    _skip_if_core_missing()

    binaries = [
        b"Hello",
        b"\x00\x01\x02\x03\xff",
        b"",
        b"\xde\xad\xbe\xef",
        bytes(range(256)),
    ]

    for b in binaries:
        text = ttoon.dumps(b)
        restored = ttoon.loads(text)
        assert restored == b
        assert isinstance(restored, bytes)


def test_uuid_type_roundtrip():
    """Round-trip UUID values."""
    _skip_if_core_missing()
    import uuid

    uuids = [
        uuid.uuid4(),
        uuid.uuid4(),
        uuid.UUID("426ac144-7477-4a90-93de-33f879c62d4d"),
        uuid.UUID("00000000-0000-0000-0000-000000000000"),
        uuid.UUID("ffffffff-ffff-ffff-ffff-ffffffffffff"),
    ]

    for u in uuids:
        text = ttoon.dumps(u)
        restored = ttoon.loads(text)
        assert restored == u
        assert isinstance(restored, uuid.UUID)


def test_float_precision_roundtrip():
    """Round-trip floats without losing precision."""
    _skip_if_core_missing()

    floats = [
        1.7976931348623157e308,
        2.2250738585072014e-308,
        3.141592653589793,
        2.718281828459045,
        1.4142135623730951,
    ]

    for f in floats:
        text = ttoon.dumps(f)
        restored = ttoon.loads(text)
        assert restored == f


def test_string_special_characters_roundtrip():
    """Round-trip strings that contain special characters."""
    _skip_if_core_missing()

    strings = [
        "",
        "Hello \"World\"",
        "Line1\nLine2\nLine3",
        "Tab\tSeparated\tValues",
        "Path\\to\\file",
        "Mixed\n\t\"Special\"\r\nChars",
        "Unicode: multilingual sample 🎉",
    ]

    for s in strings:
        text = ttoon.dumps(s)
        restored = ttoon.loads(text)
        assert restored == s
        assert isinstance(restored, str)
