from __future__ import annotations

from collections.abc import Iterable, Iterator, Mapping
from dataclasses import dataclass, replace


@dataclass(frozen=True)
class _FieldTypeSpec:
    kind: str
    nullable_flag: bool = False
    precision: int | None = None
    scale: int | None = None
    has_tz: bool | None = None

    def nullable(self) -> "_FieldTypeSpec":
        return replace(self, nullable_flag=True)

    def export(self) -> dict[str, object]:
        spec: dict[str, object] = {
            "type": self.kind,
            "nullable": self.nullable_flag,
        }
        if self.precision is not None:
            spec["precision"] = self.precision
        if self.scale is not None:
            spec["scale"] = self.scale
        if self.has_tz is not None:
            spec["has_tz"] = self.has_tz
        return spec

    @property
    def codec_key(self) -> str:
        return self.kind


class _TypesNamespace:
    string = _FieldTypeSpec("string")
    int = _FieldTypeSpec("int")
    float = _FieldTypeSpec("float")
    bool = _FieldTypeSpec("bool")
    date = _FieldTypeSpec("date")
    time = _FieldTypeSpec("time")
    datetime = _FieldTypeSpec("datetime", has_tz=True)
    datetime_naive = _FieldTypeSpec("datetime", has_tz=False)
    uuid = _FieldTypeSpec("uuid")
    binary = _FieldTypeSpec("binary")

    def decimal(self, precision: int, scale: int) -> _FieldTypeSpec:
        return _FieldTypeSpec("decimal", precision=int(precision), scale=int(scale))


types = _TypesNamespace()


class StreamSchema(Mapping[str, _FieldTypeSpec]):
    def __init__(
        self,
        fields: Mapping[str, _FieldTypeSpec] | Iterable[tuple[str, _FieldTypeSpec]],
    ) -> None:
        items = fields.items() if isinstance(fields, Mapping) else fields
        normalized: dict[str, _FieldTypeSpec] = {}
        for name, field_type in items:
            if not isinstance(name, str):
                raise TypeError("stream schema field name must be str")
            if not isinstance(field_type, _FieldTypeSpec):
                raise TypeError(
                    "stream schema field type must be built from ttoon.types"
                )
            if name in normalized:
                raise ValueError(f"duplicate stream schema field '{name}'")
            normalized[name] = field_type
        if not normalized:
            raise ValueError("stream schema must contain at least one field")
        self._fields = normalized

    def __getitem__(self, key: str) -> _FieldTypeSpec:
        return self._fields[key]

    def __iter__(self) -> Iterator[str]:
        return iter(self._fields)

    def __len__(self) -> int:
        return len(self._fields)

    def __repr__(self) -> str:
        inner = ", ".join(f"{name}={field_type!r}" for name, field_type in self.items())
        return f"StreamSchema({inner})"

    def export(self) -> list[tuple[str, dict[str, object]]]:
        return [(name, field_type.export()) for name, field_type in self.items()]
