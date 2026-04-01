from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from benchmarks.python.common.native_source import load_native_source


def load_json_value(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def load_object_source(path: Path) -> Any:
    return load_native_source(path)


def load_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def load_arrow_table(path: Path) -> Any:
    import pyarrow as pa
    import pyarrow.ipc as ipc

    data = path.read_bytes()
    buffer = pa.py_buffer(data)

    try:
        with ipc.open_file(buffer) as reader:
            return reader.read_all()
    except pa.ArrowInvalid:
        with ipc.open_stream(buffer) as reader:
            return reader.read_all()
