# Shared test cases (roundtrip, parsing, etc.) are in tests/fixtures/
# This file contains only tests unique to the Python package.

import pytest
import ttoon


def test_version():
    assert ttoon.__version__ == "0.1.0"


def test_loads_default_mode_is_compat_for_bare_tokens():
    assert ttoon.loads("@") == "@"
    assert ttoon.loads("#") == "#"
    assert ttoon.loads("$") == "$"
    assert ttoon.loads("`") == "`"


def test_loads_can_explicitly_switch_to_strict():
    with pytest.raises(ValueError, match="unknown bare token"):
        ttoon.loads("@", mode="strict")


def test_dumps_rejects_unknown_delimiter_early():
    with pytest.raises(ValueError, match="unknown delimiter"):
        ttoon.dumps({"a": 1}, delimiter=";")


def test_dumps_rejects_unknown_binary_format_early():
    with pytest.raises(ValueError, match="unknown binary_format"):
        ttoon.dumps(b"abc", binary_format="raw")
