__version__ = "0.1.0"

try:
    from . import _core  # type: ignore[attr-defined]
except ImportError:
    _core = None

if _core is not None and hasattr(_core, "TranscodeError"):
    TranscodeError = _core.TranscodeError
else:
    class TranscodeError(ValueError):
        pass


def dumps(
    obj: object,
    delimiter: str = ",",
    indent_size: int | None = None,
    binary_format: str | None = None,
) -> str:
    """Serialize a Python object (or Polars DataFrame / PyArrow Table) to T-TOON text.

    Input types are detected automatically:
    - Polars DataFrame -> converted to a PyArrow Table, then serialized through the Arrow path (zero-copy)
    - PyArrow Table/RecordBatch -> serialized through the Arrow path (zero-copy, detected by Rust)
    - Regular Python objects (dict, list, int, float, str, bool, None, etc.) -> serialized through the Node path

    delimiter: field delimiter for tabular/array output, "," (default) / "\\t" / "|"
    indent_size: indentation width, default is 2
    binary_format: binary output format, "hex" (default) / "b64"
    """
    if _core is None:
        raise RuntimeError("core extension not available")

    d = delimiter if delimiter != "," else None  # None 使用 Rust 預設值

    if (
        obj.__class__.__name__ == "DataFrame"
        and obj.__class__.__module__ == "polars.dataframe.frame"
    ):
        arrow_table = obj.to_arrow()  # type: ignore[union-attr]
        return _core.dumps(arrow_table, d, indent_size, binary_format)

    return _core.dumps(obj, d, indent_size, binary_format)


def loads(text: str, mode: str | None = None) -> object:
    """Deserialize T-TOON/T-JSON text into a Python object.

    The format is detected automatically (T-JSON or T-TOON), including top-level scalars.
    mode: parse mode for T-TOON / typed_unit input, "compat" (default) or "strict".
          The T-JSON path is always strict and does not use this parameter.
    """
    if _core is None:
        raise RuntimeError("core extension not available")
    return _core.loads(text, mode)


def read_arrow(text: str) -> "pyarrow.Table":  # type: ignore[name-defined]
    """Parse T-TOON/T-JSON text into a PyArrow Table through the zero-copy Arrow path.

    Input must be a 2D arrowable structure and field values must be scalar.
    - T-JSON list of objects: sparse rows are allowed and missing keys are treated as null
    - T-TOON tabular: schema follows the header field order and width
    - T-TOON structure: sparse schema inference is not supported
    """
    if _core is None:
        raise RuntimeError("core extension not available")
    import pyarrow
    rb = _core.read_arrow(text)  # pyarrow RecordBatch
    return pyarrow.Table.from_batches([rb])


def to_tjson(
    obj: object,
    binary_format: str | None = None,
) -> str:
    """Serialize a Python object to T-JSON text.

    T-JSON is a JSON-like format that uses {}/[] structure syntax and typed value syntax
    for scalars such as uuid(...), 123.45m, and 2026-01-20.

    Use stringify_arrow_tjson() for Arrow input.

    binary_format: binary output format, "hex" (default) / "b64"
    """
    if _core is None:
        raise RuntimeError("core extension not available")
    return _core.to_tjson(obj, binary_format)


def stringify_arrow_tjson(
    obj: object,
    binary_format: str | None = None,
) -> str:
    """Serialize a PyArrow Table/RecordBatch (or Polars DataFrame) to T-JSON text.

    Dedicated Arrow -> T-JSON entry point. Output is a JSON array of objects: [{...}, ...]

    Input types are detected automatically:
    - Polars DataFrame -> converted to a PyArrow Table first
    - PyArrow Table/RecordBatch -> serialized directly

    binary_format: binary output format, "hex" (default) / "b64"
    """
    if _core is None:
        raise RuntimeError("core extension not available")

    if (
        obj.__class__.__name__ == "DataFrame"
        and obj.__class__.__module__ == "polars.dataframe.frame"
    ):
        arrow_table = obj.to_arrow()  # type: ignore[union-attr]
        return _core.stringify_arrow_tjson(arrow_table, binary_format)

    return _core.stringify_arrow_tjson(obj, binary_format)


def tjson_to_ttoon(
    text: str,
    *,
    delimiter: str = ",",
    indent_size: int | None = None,
    binary_format: str | None = None,
) -> str:
    """Directly transcode T-JSON to T-TOON without converting through a Python object.

    Uses the dedicated T-JSON parser -> IR -> T-TOON serializer pipeline in one pass.
    The T-JSON path is always strict and does not accept a mode parameter.

    delimiter: field delimiter for tabular/array output, "," (default) / "\\t" / "|"
    indent_size: indentation width, default is 2
    binary_format: binary output format, "hex" (default) / "b64"
    """
    if _core is None:
        raise RuntimeError("core extension not available")
    d = delimiter if delimiter != "," else None
    return _core.tjson_to_ttoon(text, d, indent_size, binary_format)


def ttoon_to_tjson(
    text: str,
    *,
    mode: str = "compat",
    binary_format: str | None = None,
) -> str:
    """Directly transcode T-TOON to T-JSON without converting through a Python object.

    Uses the T-TOON parser -> IR -> T-JSON serializer pipeline in one pass.
    mode: T-TOON parse mode, "compat" (default) or "strict".

    binary_format: binary output format, "hex" (default) / "b64"
    """
    if _core is None:
        raise RuntimeError("core extension not available")
    m = mode if mode != "compat" else None
    return _core.ttoon_to_tjson(text, m, binary_format)


def detect_format(text: str) -> str:
    """Detect the input format and return "tjson", "ttoon", or "typed_unit".

    - "tjson": T-JSON input (`{` or `[` that is not a TOON header)
    - "ttoon": T-TOON input (tabular header `[N]:` / `[N]{fields}:`)
    - "typed_unit": a single typed scalar or bare value such as `42`, `true`, or `null`
    """
    if _core is None:
        raise RuntimeError("core extension not available")
    return _core.detect_format(text)


from ._schema import StreamSchema, types
from ._streaming import (
    ArrowStreamReader,
    ArrowStreamWriter,
    StreamReader,
    StreamResult,
    StreamWriter,
    TjsonArrowStreamReader,
    TjsonArrowStreamWriter,
    TjsonStreamReader,
    TjsonStreamWriter,
    stream_read,
    stream_read_arrow,
    stream_read_arrow_tjson,
    stream_read_tjson,
    stream_writer,
    stream_writer_arrow,
    stream_writer_arrow_tjson,
    stream_writer_tjson,
    use,
)


__all__ = [
    "ArrowStreamReader",
    "ArrowStreamWriter",
    "StreamReader",
    "StreamResult",
    "StreamSchema",
    "StreamWriter",
    "TjsonArrowStreamReader",
    "TjsonArrowStreamWriter",
    "TjsonStreamReader",
    "TjsonStreamWriter",
    "TranscodeError",
    "detect_format",
    "dumps",
    "loads",
    "read_arrow",
    "stream_read",
    "stream_read_arrow",
    "stream_read_arrow_tjson",
    "stream_read_tjson",
    "stream_writer",
    "stream_writer_arrow",
    "stream_writer_arrow_tjson",
    "stream_writer_tjson",
    "stringify_arrow_tjson",
    "to_tjson",
    "tjson_to_ttoon",
    "ttoon_to_tjson",
    "types",
    "use",
]
