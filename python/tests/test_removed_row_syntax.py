"""
Compatibility tests for the removed row-like syntax.

v003 removed the legacy comma-separated row syntax. Inputs such as `1, 2, 3`
are no longer valid and must raise ValueError.

This file verifies:
1. Removed row-like inputs are rejected.
2. T-TOON tabular Arrow round-trips still work.
"""
from typing import Any

import pytest
import ttoon


def _skip_if_core_missing():
    if ttoon._core is None:
        pytest.skip("core extension not available")


def test_comma_separated_ints_rejected():
    """Reject bare comma-separated integer rows."""
    _skip_if_core_missing()
    with pytest.raises(ValueError):
        ttoon.loads("1, 2, 3")


def test_multiline_comma_separated_rejected():
    """Reject multiline comma-separated values."""
    _skip_if_core_missing()
    with pytest.raises(ValueError):
        ttoon.loads("1, 2\n3, 4")


def test_string_rows_rejected():
    """Reject bare comma-separated string rows."""
    _skip_if_core_missing()
    with pytest.raises(ValueError):
        ttoon.loads('"a", "b", "c"')


def test_tabular_arrow_roundtrip():
    """Round-trip T-TOON tabular data through dumps() -> read_arrow()."""
    _skip_if_core_missing()
    pyarrow = pytest.importorskip("pyarrow")

    data = [{"name": "Alice", "score": 95}, {"name": "Bob", "score": 87}]
    text = ttoon.dumps(data)
    table: Any = ttoon.read_arrow(text)

    assert isinstance(table, pyarrow.Table)
    assert table.num_rows == 2
    assert "name" in table.schema.names
    assert "score" in table.schema.names
