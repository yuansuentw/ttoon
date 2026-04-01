import pytest
import ttoon


def _skip_if_core_missing():
    if ttoon._core is None:
        pytest.skip("core extension not available")


def test_tjson_to_ttoon_direct_helper_basic():
    _skip_if_core_missing()
    text = ttoon.tjson_to_ttoon('{"name": "Alice", "age": 30}')
    assert 'name: "Alice"' in text
    assert "age: 30" in text


def test_tjson_to_ttoon_raises_structured_transcode_error():
    _skip_if_core_missing()
    with pytest.raises(ttoon.TranscodeError) as exc_info:
        ttoon.tjson_to_ttoon("key: value")

    err = exc_info.value
    assert err.operation == "tjson_to_ttoon"
    assert err.phase == "parse"
    assert err.source_kind == "parse"
    assert err.source["kind"] == "parse"
    assert err.source["message"]


def test_ttoon_to_tjson_strict_raises_structured_transcode_error():
    _skip_if_core_missing()
    with pytest.raises(ttoon.TranscodeError) as exc_info:
        ttoon.ttoon_to_tjson("key: hello", mode="strict")

    err = exc_info.value
    assert err.operation == "ttoon_to_tjson"
    assert err.phase == "parse"
    assert err.source_kind in {"lex", "parse"}
    assert err.source["kind"] == err.source_kind
    assert "unknown bare token" in err.source["message"]
