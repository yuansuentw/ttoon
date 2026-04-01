from __future__ import annotations

from collections.abc import Iterator, Mapping
from dataclasses import dataclass
from typing import Any

from . import _core
from ._schema import StreamSchema, _FieldTypeSpec


@dataclass(frozen=True)
class _CodecAdapter:
    encode: Any | None
    decode: Any | None


_GLOBAL_CODECS: dict[str, _CodecAdapter] = {}


class StreamResult:
    def __init__(self, rows_emitted: int) -> None:
        self.rows_emitted = rows_emitted

    def __repr__(self) -> str:
        return f"StreamResult(rows_emitted={self.rows_emitted})"


if _core is not None and hasattr(_core, "StreamResult"):
    StreamResult = _core.StreamResult


def _ensure_core() -> None:
    if _core is None:
        raise RuntimeError("core extension not available")


def _coerce_schema(schema: StreamSchema | Mapping[str, _FieldTypeSpec]) -> StreamSchema:
    if isinstance(schema, StreamSchema):
        return schema
    if isinstance(schema, Mapping):
        return StreamSchema(schema)
    raise TypeError("schema must be a StreamSchema or mapping[str, field_type]")


def _coerce_codec(codec: object) -> _CodecAdapter:
    if isinstance(codec, Mapping):
        encode = codec.get("encode")
        decode = codec.get("decode")
    else:
        encode = getattr(codec, "encode", None)
        decode = getattr(codec, "decode", None)

    if encode is not None and not callable(encode):
        raise TypeError("codec.encode must be callable")
    if decode is not None and not callable(decode):
        raise TypeError("codec.decode must be callable")
    if encode is None and decode is None:
        raise TypeError("codec must provide encode() and/or decode()")

    return _CodecAdapter(encode=encode, decode=decode)


def _snapshot_codecs(codecs: Mapping[str, object] | None) -> dict[str, _CodecAdapter]:
    snapshot = dict(_GLOBAL_CODECS)
    if codecs is None:
        return snapshot
    for key, codec in codecs.items():
        snapshot[key] = _coerce_codec(codec)
    return snapshot


def use(codecs: Mapping[str, object]) -> None:
    for key, codec in codecs.items():
        _GLOBAL_CODECS[key] = _coerce_codec(codec)


def _apply_encode(
    field_type: _FieldTypeSpec | None,
    value: Any,
    codecs: Mapping[str, _CodecAdapter],
) -> Any:
    if field_type is None or value is None:
        return value
    codec = codecs.get(field_type.codec_key)
    if codec is None or codec.encode is None:
        return value
    return codec.encode(value)


def _apply_decode(
    field_type: _FieldTypeSpec | None,
    value: Any,
    codecs: Mapping[str, _CodecAdapter],
) -> Any:
    if field_type is None or value is None:
        return value
    codec = codecs.get(field_type.codec_key)
    if codec is None or codec.decode is None:
        return value
    return codec.decode(value)


def _encode_row(
    schema: StreamSchema,
    row: Mapping[str, Any],
    codecs: Mapping[str, _CodecAdapter],
) -> dict[str, Any]:
    return {
        key: _apply_encode(schema.get(key), value, codecs)
        for key, value in row.items()
    }


def _decode_row(
    schema: StreamSchema,
    row: Mapping[str, Any],
    codecs: Mapping[str, _CodecAdapter],
) -> dict[str, Any]:
    return {
        key: _apply_decode(schema.get(key), value, codecs)
        for key, value in row.items()
    }


class StreamReader(Iterator[dict[str, Any]]):
    def __init__(
        self,
        source: object,
        *,
        schema: StreamSchema | Mapping[str, _FieldTypeSpec],
        mode: str | None = None,
        codecs: Mapping[str, object] | None = None,
    ) -> None:
        _ensure_core()
        self._schema = _coerce_schema(schema)
        self._codecs = _snapshot_codecs(codecs)
        self._reader = _core.StreamReader(source, self._schema.export(), mode)

    def __iter__(self) -> "StreamReader":
        return self

    def __next__(self) -> dict[str, Any]:
        row = next(self._reader)
        return _decode_row(self._schema, row, self._codecs)


class StreamWriter:
    def __init__(
        self,
        sink: object,
        *,
        schema: StreamSchema | Mapping[str, _FieldTypeSpec],
        delimiter: str = ",",
        binary_format: str | None = None,
        codecs: Mapping[str, object] | None = None,
    ) -> None:
        _ensure_core()
        self._schema = _coerce_schema(schema)
        self._codecs = _snapshot_codecs(codecs)
        self._writer = _core.StreamWriter(
            sink,
            self._schema.export(),
            None if delimiter == "," else delimiter,
            binary_format,
        )
        self._result: StreamResult | None = None

    @property
    def result(self) -> StreamResult | None:
        return self._result

    def write(self, row: Mapping[str, Any]) -> None:
        encoded = _encode_row(self._schema, row, self._codecs)
        self._writer.write(encoded)

    def close(self) -> StreamResult:
        if self._result is None:
            self._result = self._writer.close()
        return self._result

    def __enter__(self) -> "StreamWriter":
        return self

    def __exit__(self, exc_type: object, exc: object, tb: object) -> bool:
        if exc_type is None:
            self.close()
        return False


class ArrowStreamReader(Iterator[Any]):
    def __init__(
        self,
        source: object,
        *,
        schema: StreamSchema | Mapping[str, _FieldTypeSpec],
        batch_size: int = 1024,
        mode: str | None = None,
    ) -> None:
        _ensure_core()
        self._schema = _coerce_schema(schema)
        self._reader = _core.ArrowStreamReader(
            source,
            self._schema.export(),
            batch_size,
            mode,
        )

    def __iter__(self) -> "ArrowStreamReader":
        return self

    def __next__(self) -> Any:
        return next(self._reader)


class ArrowStreamWriter:
    def __init__(
        self,
        sink: object,
        *,
        schema: StreamSchema | Mapping[str, _FieldTypeSpec],
        delimiter: str = ",",
        binary_format: str | None = None,
    ) -> None:
        _ensure_core()
        self._schema = _coerce_schema(schema)
        self._writer = _core.ArrowStreamWriter(
            sink,
            self._schema.export(),
            None if delimiter == "," else delimiter,
            binary_format,
        )
        self._result: StreamResult | None = None

    @property
    def result(self) -> StreamResult | None:
        return self._result

    def write_batch(self, batch: object) -> None:
        self._writer.write_batch(batch)

    def close(self) -> StreamResult:
        if self._result is None:
            self._result = self._writer.close()
        return self._result

    def __enter__(self) -> "ArrowStreamWriter":
        return self

    def __exit__(self, exc_type: object, exc: object, tb: object) -> bool:
        if exc_type is None:
            self.close()
        return False


class TjsonStreamReader(Iterator[dict[str, Any]]):
    def __init__(
        self,
        source: object,
        *,
        schema: StreamSchema | Mapping[str, _FieldTypeSpec],
        mode: str | None = None,
        codecs: Mapping[str, object] | None = None,
    ) -> None:
        _ensure_core()
        self._schema = _coerce_schema(schema)
        self._codecs = _snapshot_codecs(codecs)
        self._reader = _core.TjsonStreamReader(source, self._schema.export(), mode)

    def __iter__(self) -> "TjsonStreamReader":
        return self

    def __next__(self) -> dict[str, Any]:
        row = next(self._reader)
        return _decode_row(self._schema, row, self._codecs)


class TjsonStreamWriter:
    def __init__(
        self,
        sink: object,
        *,
        schema: StreamSchema | Mapping[str, _FieldTypeSpec],
        binary_format: str | None = None,
        codecs: Mapping[str, object] | None = None,
    ) -> None:
        _ensure_core()
        self._schema = _coerce_schema(schema)
        self._codecs = _snapshot_codecs(codecs)
        self._writer = _core.TjsonStreamWriter(
            sink,
            self._schema.export(),
            binary_format,
        )
        self._result: StreamResult | None = None

    @property
    def result(self) -> StreamResult | None:
        return self._result

    def write(self, row: Mapping[str, Any]) -> None:
        encoded = _encode_row(self._schema, row, self._codecs)
        self._writer.write(encoded)

    def close(self) -> StreamResult:
        if self._result is None:
            self._result = self._writer.close()
        return self._result

    def __enter__(self) -> "TjsonStreamWriter":
        return self

    def __exit__(self, exc_type: object, exc: object, tb: object) -> bool:
        if exc_type is None:
            self.close()
        return False


class TjsonArrowStreamReader(Iterator[Any]):
    def __init__(
        self,
        source: object,
        *,
        schema: StreamSchema | Mapping[str, _FieldTypeSpec],
        batch_size: int = 1024,
        mode: str | None = None,
    ) -> None:
        _ensure_core()
        self._schema = _coerce_schema(schema)
        self._reader = _core.TjsonArrowStreamReader(
            source,
            self._schema.export(),
            batch_size,
            mode,
        )

    def __iter__(self) -> "TjsonArrowStreamReader":
        return self

    def __next__(self) -> Any:
        return next(self._reader)


class TjsonArrowStreamWriter:
    def __init__(
        self,
        sink: object,
        *,
        schema: StreamSchema | Mapping[str, _FieldTypeSpec],
        binary_format: str | None = None,
    ) -> None:
        _ensure_core()
        self._schema = _coerce_schema(schema)
        self._writer = _core.TjsonArrowStreamWriter(
            sink,
            self._schema.export(),
            binary_format,
        )
        self._result: StreamResult | None = None

    @property
    def result(self) -> StreamResult | None:
        return self._result

    def write_batch(self, batch: object) -> None:
        self._writer.write_batch(batch)

    def close(self) -> StreamResult:
        if self._result is None:
            self._result = self._writer.close()
        return self._result

    def __enter__(self) -> "TjsonArrowStreamWriter":
        return self

    def __exit__(self, exc_type: object, exc: object, tb: object) -> bool:
        if exc_type is None:
            self.close()
        return False


def stream_read(
    source: object,
    *,
    schema: StreamSchema | Mapping[str, _FieldTypeSpec],
    mode: str | None = None,
    codecs: Mapping[str, object] | None = None,
) -> StreamReader:
    return StreamReader(source, schema=schema, mode=mode, codecs=codecs)


def stream_writer(
    sink: object,
    *,
    schema: StreamSchema | Mapping[str, _FieldTypeSpec],
    delimiter: str = ",",
    binary_format: str | None = None,
    codecs: Mapping[str, object] | None = None,
) -> StreamWriter:
    return StreamWriter(
        sink,
        schema=schema,
        delimiter=delimiter,
        binary_format=binary_format,
        codecs=codecs,
    )


def stream_read_arrow(
    source: object,
    *,
    schema: StreamSchema | Mapping[str, _FieldTypeSpec],
    batch_size: int = 1024,
    mode: str | None = None,
) -> ArrowStreamReader:
    return ArrowStreamReader(source, schema=schema, batch_size=batch_size, mode=mode)


def stream_writer_arrow(
    sink: object,
    *,
    schema: StreamSchema | Mapping[str, _FieldTypeSpec],
    delimiter: str = ",",
    binary_format: str | None = None,
) -> ArrowStreamWriter:
    return ArrowStreamWriter(
        sink,
        schema=schema,
        delimiter=delimiter,
        binary_format=binary_format,
    )


def stream_read_tjson(
    source: object,
    *,
    schema: StreamSchema | Mapping[str, _FieldTypeSpec],
    mode: str | None = None,
    codecs: Mapping[str, object] | None = None,
) -> TjsonStreamReader:
    return TjsonStreamReader(source, schema=schema, mode=mode, codecs=codecs)


def stream_writer_tjson(
    sink: object,
    *,
    schema: StreamSchema | Mapping[str, _FieldTypeSpec],
    binary_format: str | None = None,
    codecs: Mapping[str, object] | None = None,
) -> TjsonStreamWriter:
    return TjsonStreamWriter(
        sink,
        schema=schema,
        binary_format=binary_format,
        codecs=codecs,
    )


def stream_read_arrow_tjson(
    source: object,
    *,
    schema: StreamSchema | Mapping[str, _FieldTypeSpec],
    batch_size: int = 1024,
    mode: str | None = None,
) -> TjsonArrowStreamReader:
    return TjsonArrowStreamReader(
        source,
        schema=schema,
        batch_size=batch_size,
        mode=mode,
    )


def stream_writer_arrow_tjson(
    sink: object,
    *,
    schema: StreamSchema | Mapping[str, _FieldTypeSpec],
    binary_format: str | None = None,
) -> TjsonArrowStreamWriter:
    return TjsonArrowStreamWriter(
        sink,
        schema=schema,
        binary_format=binary_format,
    )
