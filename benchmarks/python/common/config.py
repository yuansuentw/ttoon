from __future__ import annotations

from collections.abc import Iterable

LANGUAGES = ("python", "js", "rust")
VARIANTS = ("js-basic", "extended")
SHAPES = ("structure", "tabular")
SIZES = ("10k", "100k", "1m")

DEFAULT_WARMUPS = 2
DEFAULT_ITERATIONS = 20
RUNNER_SCHEMA_VERSION = 1

CASE_MATRIX: dict[str, dict[str, dict[str, tuple[str, ...]]]] = {
    "python": {
        "js-basic": {
            "structure": (
                "json_serialize",
                "json_deserialize",
                "tjson_serialize",
                "tjson_deserialize",
                "ttoon_serialize",
                "ttoon_deserialize",
            ),
            "tabular": (
                "arrow_tjson_serialize",
                "arrow_tjson_deserialize",
                "arrow_tjson_stream_serialize",
                "arrow_tjson_stream_deserialize",
                "arrow_ttoon_serialize",
                "arrow_ttoon_deserialize",
                "arrow_ttoon_stream_serialize",
                "arrow_ttoon_stream_deserialize",
            ),
        },
        "extended": {
            "structure": (
                "ttoon_serialize",
                "ttoon_deserialize",
            ),
            "tabular": (
                "arrow_ttoon_serialize",
                "arrow_ttoon_deserialize",
                "arrow_tjson_serialize",
                "arrow_tjson_deserialize",
                "arrow_tjson_stream_serialize",
                "arrow_tjson_stream_deserialize",
                "arrow_ttoon_stream_serialize",
                "arrow_ttoon_stream_deserialize",
            ),
        },
    },
    "js": {
        "js-basic": {
            "structure": (
                "json_serialize",
                "json_deserialize",
                "tjson_serialize",
                "tjson_deserialize",
                "ttoon_serialize",
                "ttoon_deserialize",
                "toon_serialize",
                "toon_deserialize",
            ),
            "tabular": (
                "arrow_tjson_serialize",
                "arrow_tjson_deserialize",
                "arrow_ttoon_serialize",
                "arrow_ttoon_deserialize",
            ),
        },
        "extended": {
            "structure": (
                "ttoon_serialize",
                "ttoon_deserialize",
            ),
            "tabular": (
                "arrow_ttoon_serialize",
                "arrow_ttoon_deserialize",
                "arrow_tjson_serialize",
                "arrow_tjson_deserialize",
            ),
        },
    },
    "rust": {
        "js-basic": {
            "structure": (
                "json_serialize",
                "json_deserialize",
                "tjson_serialize",
                "tjson_deserialize",
                "ttoon_serialize",
                "ttoon_deserialize",
            ),
            "tabular": (
                "arrow_tjson_serialize",
                "arrow_tjson_deserialize",
                "arrow_tjson_stream_serialize",
                "arrow_tjson_stream_deserialize",
                "arrow_ttoon_serialize",
                "arrow_ttoon_deserialize",
                "arrow_ttoon_stream_serialize",
                "arrow_ttoon_stream_deserialize",
            ),
        },
        "extended": {
            "structure": (
                "ttoon_serialize",
                "ttoon_deserialize",
            ),
            "tabular": (
                "arrow_ttoon_serialize",
                "arrow_ttoon_deserialize",
                "arrow_tjson_serialize",
                "arrow_tjson_deserialize",
                "arrow_tjson_stream_serialize",
                "arrow_tjson_stream_deserialize",
                "arrow_ttoon_stream_serialize",
                "arrow_ttoon_stream_deserialize",
            ),
        },
    },
}


def list_languages(filters: Iterable[str] | None = None) -> list[str]:
    return _apply_filters(LANGUAGES, filters)


def list_cases(language: str, variant: str | None = None, shape: str | None = None) -> list[str]:
    if language not in CASE_MATRIX:
        return []

    variants = (variant,) if variant else CASE_MATRIX[language].keys()
    results: list[str] = []
    for variant_name in variants:
        shape_map = CASE_MATRIX[language].get(variant_name, {})
        shapes = (shape,) if shape else shape_map.keys()
        for shape_name in shapes:
            results.extend(shape_map.get(shape_name, ()))
    return results


def case_direction(case_name: str) -> str:
    if case_name.endswith("_serialize"):
        return "serialize"
    if case_name.endswith("_deserialize"):
        return "deserialize"
    return "unknown"


def choose_baseline(case_names: set[str], direction: str) -> str | None:
    if direction not in {"serialize", "deserialize"}:
        return None

    candidate = f"json_{direction}"
    if candidate in case_names:
        return candidate
    return None


def _apply_filters(values: Iterable[str], filters: Iterable[str] | None) -> list[str]:
    items = list(values)
    if not filters:
        return items

    allowed = {value for value in filters}
    return [value for value in items if value in allowed]
