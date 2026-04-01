from __future__ import annotations

from datetime import date, datetime, time
from decimal import Decimal
from pathlib import Path
from typing import Any
import json
import uuid

WRAPPER_KIND_KEY = "$kind"
WRAPPER_VALUE_KEY = "value"


def load_native_source(path: Path) -> Any:
    return hydrate_native_value(json.loads(path.read_text(encoding="utf-8")))


def hydrate_native_value(value: Any) -> Any:
    if isinstance(value, list):
        return [hydrate_native_value(item) for item in value]

    if isinstance(value, dict):
        if _is_typed_wrapper(value):
            return _hydrate_wrapper(value)
        return {key: hydrate_native_value(item) for key, item in value.items()}

    return value


def encode_native_value(value: Any) -> Any:
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, Decimal):
        return {WRAPPER_KIND_KEY: "decimal", WRAPPER_VALUE_KEY: format(value, "f")}
    if isinstance(value, uuid.UUID):
        return {WRAPPER_KIND_KEY: "uuid", WRAPPER_VALUE_KEY: str(value)}
    if isinstance(value, datetime):
        text = value.isoformat()
        if text.endswith("+00:00"):
            text = text[:-6] + "Z"
        return {WRAPPER_KIND_KEY: "datetime", WRAPPER_VALUE_KEY: text}
    if isinstance(value, date):
        return {WRAPPER_KIND_KEY: "date", WRAPPER_VALUE_KEY: value.isoformat()}
    if isinstance(value, time):
        return {WRAPPER_KIND_KEY: "time", WRAPPER_VALUE_KEY: value.isoformat()}
    if isinstance(value, list):
        return [encode_native_value(item) for item in value]
    if isinstance(value, dict):
        return {key: encode_native_value(item) for key, item in value.items()}

    raise TypeError(f"unsupported native source value: {type(value)!r}")


def _is_typed_wrapper(value: dict[str, Any]) -> bool:
    return set(value.keys()) == {WRAPPER_KIND_KEY, WRAPPER_VALUE_KEY}


def _hydrate_wrapper(value: dict[str, Any]) -> Any:
    kind = value[WRAPPER_KIND_KEY]
    raw = value[WRAPPER_VALUE_KEY]

    if kind == "decimal":
        return Decimal(str(raw))
    if kind == "uuid":
        return uuid.UUID(str(raw))
    if kind == "date":
        return date.fromisoformat(str(raw))
    if kind == "time":
        return time.fromisoformat(str(raw))
    if kind == "datetime":
        text = str(raw)
        if text.endswith("Z"):
            text = text[:-1] + "+00:00"
        return datetime.fromisoformat(text)

    raise ValueError(f"unknown native source wrapper kind: {kind!r}")
