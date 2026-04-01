from decimal import Decimal
from typing import Any
import uuid

import pytest  # type: ignore[import-not-found]
import ttoon


def _skip_if_core_missing():
    if ttoon._core is None:
        pytest.skip("core extension not available")


def test_to_tjson_basic_roundtrip():
    _skip_if_core_missing()
    obj: dict[str, Any] = {
        "name": "Alice",
        "score": 95,
        "price": Decimal("12.50"),
        "id": uuid.UUID("426ac144-7477-4a90-93de-33f879c62d4d"),
    }

    text = ttoon.to_tjson(obj)
    assert text.startswith("{")
    assert "12.50m" in text
    assert "uuid(426ac144-7477-4a90-93de-33f879c62d4d)" in text

    restored = ttoon.loads(text)
    assert restored == obj


def test_to_tjson_exact_uuid_bypasses_python_str(monkeypatch: pytest.MonkeyPatch):
    _skip_if_core_missing()
    value = uuid.UUID("426ac144-7477-4a90-93de-33f879c62d4d")

    def fail_str(self: uuid.UUID) -> str:
        raise AssertionError("exact uuid fast path should not call __str__")

    monkeypatch.setattr(uuid.UUID, "__str__", fail_str)

    text = ttoon.to_tjson({"id": value})
    assert "uuid(426ac144-7477-4a90-93de-33f879c62d4d)" in text


def test_to_tjson_uuid_subclass_falls_back_to_python_str():
    _skip_if_core_missing()

    class TrackingUUID(uuid.UUID):
        called = 0

        def __str__(self) -> str:
            type(self).called += 1
            return super().__str__()

    value = TrackingUUID("426ac144-7477-4a90-93de-33f879c62d4d")
    text = ttoon.to_tjson({"id": value})

    assert "uuid(426ac144-7477-4a90-93de-33f879c62d4d)" in text
    assert TrackingUUID.called == 1


def test_stringify_arrow_tjson():
    _skip_if_core_missing()
    pyarrow = pytest.importorskip("pyarrow")

    table = pyarrow.table(
        {
            "name": pyarrow.array(["Alice", "Bob"], type=pyarrow.string()),
            "score": pyarrow.array([95, 87], type=pyarrow.int64()),
        }
    )

    text = ttoon.stringify_arrow_tjson(table)
    assert text.startswith("[")
    assert "\"name\"" in text
    assert "\"Alice\"" in text

    restored = ttoon.loads(text)
    assert restored == [
        {"name": "Alice", "score": 95},
        {"name": "Bob", "score": 87},
    ]


def test_to_tjson_rejects_arrow_input():
    _skip_if_core_missing()
    pyarrow = pytest.importorskip("pyarrow")

    table = pyarrow.table(
        {
            "name": pyarrow.array(["Alice"], type=pyarrow.string()),
        }
    )

    with pytest.raises(TypeError):
        ttoon.to_tjson(table)
