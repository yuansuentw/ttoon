"""
Integration tests for the zero-copy architecture.

These tests verify zero-copy round-trips for Polars DataFrame and PyArrow Table inputs.
"""
from typing import Any, cast

import pytest
import ttoon


def _skip_if_core_missing():
    if ttoon._core is None:
        pytest.skip("core extension not available")


def test_polars_zero_copy_roundtrip_basic():
    """Round-trip a basic Polars DataFrame through the zero-copy path."""
    _skip_if_core_missing()
    polars = pytest.importorskip("polars")

    df = polars.DataFrame({
        "int_col": [1, 2, 3, 4, 5],
        "float_col": [1.1, 2.2, 3.3, 4.4, 5.5],
        "str_col": ["a", "b", "c", "d", "e"],
        "bool_col": [True, False, True, False, True],
    })

    text = ttoon.dumps(df)
    restored: Any = polars.from_arrow(ttoon.read_arrow(text))

    assert isinstance(restored, polars.DataFrame)
    assert restored.schema == df.schema
    assert restored.equals(df)


def test_polars_zero_copy_roundtrip_large():
    """Round-trip a large Polars DataFrame through the zero-copy path."""
    _skip_if_core_missing()
    polars = pytest.importorskip("polars")

    df = polars.DataFrame({
        "int_col": list(range(100_000)),
        "float_col": [i * 0.1 for i in range(100_000)],
        "str_col": [f"row_{i}" for i in range(100_000)],
    })

    text = ttoon.dumps(df)
    restored: Any = polars.from_arrow(ttoon.read_arrow(text))

    assert isinstance(restored, polars.DataFrame)
    assert restored.schema == df.schema
    assert restored.equals(df)


def test_polars_zero_copy_with_nulls():
    """Round-trip a Polars DataFrame with null values through the zero-copy path."""
    _skip_if_core_missing()
    polars = pytest.importorskip("polars")

    df = polars.DataFrame({
        "int_col": [1, None, 3, None, 5],
        "float_col": [1.1, 2.2, None, 4.4, None],
        "str_col": ["a", None, "c", None, "e"],
    })

    text = ttoon.dumps(df)
    restored: Any = polars.from_arrow(ttoon.read_arrow(text))

    assert isinstance(restored, polars.DataFrame)
    assert restored.schema == df.schema
    assert restored.equals(df)


def test_arrow_zero_copy_roundtrip_basic():
    """Round-trip a basic Arrow Table through the zero-copy path."""
    _skip_if_core_missing()
    pyarrow = pytest.importorskip("pyarrow")

    table = pyarrow.table({
        "int_col": pyarrow.array([1, 2, 3, 4, 5], type=pyarrow.int64()),
        "float_col": pyarrow.array([1.1, 2.2, 3.3, 4.4, 5.5], type=pyarrow.float64()),
        "str_col": pyarrow.array(["a", "b", "c", "d", "e"], type=pyarrow.string()),
        "bool_col": pyarrow.array([True, False, True, False, True], type=pyarrow.bool_()),
    })

    text = ttoon.dumps(table)
    restored: Any = ttoon.read_arrow(text)

    assert isinstance(restored, pyarrow.Table)
    assert restored.schema.types == table.schema.types
    assert restored.equals(table)


def test_arrow_zero_copy_roundtrip_large():
    """Round-trip a large Arrow Table through the zero-copy path."""
    _skip_if_core_missing()
    pyarrow = pytest.importorskip("pyarrow")

    table = pyarrow.table({
        "int_col": pyarrow.array(list(range(100_000)), type=pyarrow.int64()),
        "float_col": pyarrow.array([i * 0.1 for i in range(100_000)], type=pyarrow.float64()),
        "str_col": pyarrow.array([f"row_{i}" for i in range(100_000)], type=pyarrow.string()),
    })

    text = ttoon.dumps(table)
    restored: Any = ttoon.read_arrow(text)

    assert isinstance(restored, pyarrow.Table)
    assert restored.schema.types == table.schema.types
    assert restored.equals(table)


def test_arrow_zero_copy_with_nulls():
    """Round-trip an Arrow Table with null values through the zero-copy path."""
    _skip_if_core_missing()
    pyarrow = pytest.importorskip("pyarrow")

    table = pyarrow.table({
        "int_col": pyarrow.array([1, None, 3, None, 5], type=pyarrow.int64()),
        "float_col": pyarrow.array([1.1, 2.2, None, 4.4, None], type=pyarrow.float64()),
        "str_col": pyarrow.array(["a", None, "c", None, "e"], type=pyarrow.string()),
    })

    text = ttoon.dumps(table)
    restored: Any = ttoon.read_arrow(text)

    assert isinstance(restored, pyarrow.Table)
    assert restored.schema.types == table.schema.types
    assert restored.equals(table)


def test_polars_to_python_fallback():
    """Deserialize a Polars DataFrame into Python objects through loads()."""
    _skip_if_core_missing()
    polars = pytest.importorskip("polars")

    df = polars.DataFrame({
        "a": [1, 2, 3],
        "b": [4.0, 5.0, 6.0],
    })

    text = ttoon.dumps(df)
    restored = cast(list, ttoon.loads(text))

    # tabular 格式 → list of objects（dict）
    assert isinstance(restored, list)
    assert len(restored) == 3
    assert isinstance(restored[0], dict)


def test_arrow_to_python_fallback():
    """Deserialize an Arrow Table into Python objects through loads()."""
    _skip_if_core_missing()
    pyarrow = pytest.importorskip("pyarrow")

    table = pyarrow.table({
        "a": pyarrow.array([1, 2, 3], type=pyarrow.int64()),
        "b": pyarrow.array([4.0, 5.0, 6.0], type=pyarrow.float64()),
    })

    text = ttoon.dumps(table)
    restored = cast(list, ttoon.loads(text))

    # tabular 格式 → list of objects（dict）
    assert isinstance(restored, list)
    assert len(restored) == 3
    assert isinstance(restored[0], dict)


def test_empty_arrow_table_dumps_succeeds():
    """Serialize an empty Arrow Table while preserving tabular field metadata.

    Because the T-TOON Arrow path keeps schema separate from records, it can emit
    the `[0]{a,b}:` form even when there are zero rows.
    """
    _skip_if_core_missing()
    pyarrow = pytest.importorskip("pyarrow")

    table = pyarrow.table({
        "a": pyarrow.array([], type=pyarrow.int64()),
        "b": pyarrow.array([], type=pyarrow.float64()),
    })

    text = ttoon.dumps(table)
    assert text == "[0]{a,b}:\n"
