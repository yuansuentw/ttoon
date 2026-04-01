from __future__ import annotations

import io
from decimal import Decimal

import pytest
import ttoon


def _skip_if_core_missing() -> None:
    if ttoon._core is None:
        pytest.skip("core extension not available")


def test_stream_writer_and_reader_roundtrip() -> None:
    _skip_if_core_missing()
    schema = ttoon.StreamSchema(
        {
            "name": ttoon.types.string,
            "age": ttoon.types.int,
            "balance": ttoon.types.decimal(10, 2).nullable(),
        }
    )
    sink = io.StringIO()

    with ttoon.stream_writer(sink, schema=schema) as writer:
        writer.write(
            {
                "name": "Alice",
                "age": 30,
                "balance": Decimal("123.45"),
            }
        )

    assert sink.getvalue() == '[*]{name,age,balance}:\n"Alice", 30, 123.45m\n'
    assert writer.result is not None
    assert writer.result.rows_emitted == 1

    rows = list(ttoon.stream_read(io.StringIO(sink.getvalue()), schema=schema))
    assert rows == [
        {
            "name": "Alice",
            "age": 30,
            "balance": Decimal("123.45"),
        }
    ]


def test_stream_writer_close_without_rows_writes_empty_header() -> None:
    _skip_if_core_missing()
    schema = ttoon.StreamSchema({"name": ttoon.types.string, "age": ttoon.types.int})
    sink = io.StringIO()

    with ttoon.stream_writer(sink, schema=schema) as writer:
        pass

    assert sink.getvalue() == "[0]{name,age}:\n"
    assert writer.result is not None
    assert writer.result.rows_emitted == 0


def test_stream_writer_context_manager_does_not_close_on_exception() -> None:
    _skip_if_core_missing()
    schema = ttoon.StreamSchema({"name": ttoon.types.string})
    sink = io.StringIO()
    writer: ttoon.StreamWriter | None = None

    with pytest.raises(RuntimeError, match="boom"):
        with ttoon.stream_writer(sink, schema=schema) as writer:
            raise RuntimeError("boom")

    assert writer is not None
    assert writer.result is None
    assert sink.getvalue() == ""


def test_stream_read_arrow_batches() -> None:
    _skip_if_core_missing()
    pyarrow = pytest.importorskip("pyarrow")
    schema = ttoon.StreamSchema({"name": ttoon.types.string, "age": ttoon.types.int})
    text = '[*]{name,age}:\n"Alice",1\n"Bob",2\n'

    batches = list(
        ttoon.stream_read_arrow(io.StringIO(text), schema=schema, batch_size=1)
    )

    assert len(batches) == 2
    assert isinstance(batches[0], pyarrow.RecordBatch)
    assert batches[0].column(0).to_pylist() == ["Alice"]
    assert batches[1].column(1).to_pylist() == [2]


def test_stream_writer_arrow_writes_batches() -> None:
    _skip_if_core_missing()
    pyarrow = pytest.importorskip("pyarrow")
    schema = ttoon.StreamSchema({"name": ttoon.types.string, "age": ttoon.types.int})
    batch = pyarrow.RecordBatch.from_arrays(
        [
            pyarrow.array(["Alice", "Bob"], type=pyarrow.string()),
            pyarrow.array([1, 2], type=pyarrow.int64()),
        ],
        names=["name", "age"],
    )
    sink = io.StringIO()

    with ttoon.stream_writer_arrow(sink, schema=schema) as writer:
        writer.write_batch(batch)

    assert sink.getvalue() == '[*]{name,age}:\n"Alice", 1\n"Bob", 2\n'
    assert writer.result is not None
    assert writer.result.rows_emitted == 2


def test_stream_reader_uses_codec_snapshot() -> None:
    _skip_if_core_missing()
    schema = ttoon.StreamSchema({"amount": ttoon.types.decimal(10, 2)})
    text = "[*]{amount}:\n1.25m\n"

    class DecimalWrapper:
        def __init__(self, tag: str, value: Decimal) -> None:
            self.tag = tag
            self.value = value

    class CodecA:
        def decode(self, value: Decimal) -> DecimalWrapper:
            return DecimalWrapper("a", value)

    class CodecB:
        def decode(self, value: Decimal) -> DecimalWrapper:
            return DecimalWrapper("b", value)

    ttoon.use({"decimal": CodecA()})
    reader = ttoon.stream_read(io.StringIO(text), schema=schema)
    ttoon.use({"decimal": CodecB()})

    row = next(reader)
    assert row["amount"].tag == "a"
    assert row["amount"].value == Decimal("1.25")

    ttoon.use(
        {
            "decimal": {
                "encode": lambda value: value,
                "decode": lambda value: value,
            }
        }
    )


def test_tjson_stream_writer_and_reader_roundtrip() -> None:
    _skip_if_core_missing()
    schema = ttoon.StreamSchema(
        {
            "name": ttoon.types.string,
            "age": ttoon.types.int.nullable(),
        }
    )
    sink = io.StringIO()

    with ttoon.stream_writer_tjson(sink, schema=schema) as writer:
        writer.write({"age": None, "name": "Alice"})

    assert sink.getvalue() == '[{"name": "Alice", "age": null}]'
    assert writer.result is not None
    assert writer.result.rows_emitted == 1

    rows = list(ttoon.stream_read_tjson(io.StringIO(sink.getvalue()), schema=schema))
    assert rows == [{"name": "Alice", "age": None}]


def test_tjson_stream_reader_materializes_missing_field_to_null() -> None:
    _skip_if_core_missing()
    schema = ttoon.StreamSchema(
        {
            "name": ttoon.types.string,
            "age": ttoon.types.int.nullable(),
        }
    )

    rows = list(ttoon.stream_read_tjson(io.StringIO('[{"name": "Alice"}]'), schema=schema))
    assert rows == [{"name": "Alice", "age": None}]


def test_tjson_stream_read_arrow_batches() -> None:
    _skip_if_core_missing()
    pyarrow = pytest.importorskip("pyarrow")
    schema = ttoon.StreamSchema(
        {
            "name": ttoon.types.string,
            "age": ttoon.types.int.nullable(),
        }
    )
    text = '[{"name": "Alice", "age": 1}, {"name": "Bob", "age": null}]'

    batches = list(
        ttoon.stream_read_arrow_tjson(
            io.StringIO(text),
            schema=schema,
            batch_size=1,
        )
    )

    assert len(batches) == 2
    assert isinstance(batches[0], pyarrow.RecordBatch)
    assert batches[0].column(0).to_pylist() == ["Alice"]
    assert batches[1].column(1).to_pylist() == [None]


def test_tjson_stream_writer_arrow_writes_batches() -> None:
    _skip_if_core_missing()
    pyarrow = pytest.importorskip("pyarrow")
    schema = ttoon.StreamSchema(
        {
            "name": ttoon.types.string,
            "age": ttoon.types.int.nullable(),
        }
    )
    batch = pyarrow.RecordBatch.from_arrays(
        [
            pyarrow.array(["Alice", "Bob"], type=pyarrow.string()),
            pyarrow.array([1, None], type=pyarrow.int64()),
        ],
        names=["name", "age"],
    )
    sink = io.StringIO()

    with ttoon.stream_writer_arrow_tjson(sink, schema=schema) as writer:
        writer.write_batch(batch)

    assert sink.getvalue() == '[{"name": "Alice", "age": 1}, {"name": "Bob", "age": null}]'
    assert writer.result is not None
    assert writer.result.rows_emitted == 2


def test_tjson_stream_reader_uses_codec_snapshot() -> None:
    _skip_if_core_missing()
    schema = ttoon.StreamSchema({"amount": ttoon.types.decimal(10, 2)})
    text = '[{"amount": 1.25m}]'

    class DecimalWrapper:
        def __init__(self, tag: str, value: Decimal) -> None:
            self.tag = tag
            self.value = value

    class CodecA:
        def decode(self, value: Decimal) -> DecimalWrapper:
            return DecimalWrapper("a", value)

    class CodecB:
        def decode(self, value: Decimal) -> DecimalWrapper:
            return DecimalWrapper("b", value)

    ttoon.use({"decimal": CodecA()})
    reader = ttoon.stream_read_tjson(io.StringIO(text), schema=schema)
    ttoon.use({"decimal": CodecB()})

    row = next(reader)
    assert row["amount"].tag == "a"
    assert row["amount"].value == Decimal("1.25")

    ttoon.use(
        {
            "decimal": {
                "encode": lambda value: value,
                "decode": lambda value: value,
            }
        }
    )
