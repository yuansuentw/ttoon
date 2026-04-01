//! Cross-language shared fixture tests.
//!
//! Loads test cases from `tests/fixtures/*.json` and runs them
//! against the Rust tjson/ttoon parsers and serializers.
//! See `tests/fixtures/README.md` for the fixture schema.

use indexmap::IndexMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

use super::format_detect::{self, Format};
use super::ir::Node;
use super::tjson_parser as parser;
use super::tjson_serializer as serializer;
use super::ttoon_parser;
use super::{
    BinaryFormat, Delimiter, Error, ErrorKind, ParseMode, Result, TjsonOptions, TtoonOptions,
};

// ─── Helpers ────────────────────────────────────────────────────────────────────

fn fixtures_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.join("../../../tests/fixtures")
}

fn load_fixture(name: &str) -> Value {
    let path = fixtures_dir().join(name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", path.display(), e));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse fixture {}: {}", path.display(), e))
}

/// Convert a type-tagged JSON value to a `Node`.
fn json_to_node(v: &Value) -> Node {
    let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("null");
    match ty {
        "null" => Node::Null,
        "bool" => Node::Bool(v["value"].as_bool().unwrap()),
        "int" => {
            if let Some(n) = v["value"].as_i64() {
                Node::Int(n)
            } else if let Some(s) = v["value"].as_str() {
                Node::Int(s.parse::<i64>().unwrap())
            } else {
                panic!("Invalid int value: {:?}", v["value"]);
            }
        }
        "float" => {
            if let Some(n) = v["value"].as_f64() {
                Node::Float(n)
            } else if let Some(s) = v["value"].as_str() {
                match s {
                    "NaN" => Node::Float(f64::NAN),
                    "+Infinity" => Node::Float(f64::INFINITY),
                    "-Infinity" => Node::Float(f64::NEG_INFINITY),
                    "-0.0" => Node::Float(-0.0_f64),
                    _ => panic!("Unknown float special value: {}", s),
                }
            } else {
                panic!("Invalid float value: {:?}", v["value"]);
            }
        }
        "decimal" => Node::Decimal(v["value"].as_str().unwrap().to_string()),
        "string" => Node::String(v["value"].as_str().unwrap().to_string()),
        "date" => Node::Date(v["value"].as_str().unwrap().to_string()),
        "time" => Node::Time(v["value"].as_str().unwrap().to_string()),
        "datetime" => Node::DateTime(v["value"].as_str().unwrap().to_string()),
        "uuid" => Node::Uuid(v["value"].as_str().unwrap().to_string()),
        "binary_hex" => {
            let hex_str = v["value"].as_str().unwrap();
            if hex_str.is_empty() {
                Node::Binary(vec![])
            } else {
                let bytes: Vec<u8> = (0..hex_str.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&hex_str[i..i + 2], 16).unwrap())
                    .collect();
                Node::Binary(bytes)
            }
        }
        "binary_b64" => {
            let b64_str = v["value"].as_str().unwrap();
            if b64_str.is_empty() {
                Node::Binary(vec![])
            } else {
                use base64::Engine;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(b64_str)
                    .unwrap();
                Node::Binary(bytes)
            }
        }
        "list" => {
            let items = v["value"]
                .as_array()
                .unwrap()
                .iter()
                .map(json_to_node)
                .collect();
            Node::List(items)
        }
        "object" => {
            let obj = v["value"].as_object().unwrap();
            let mut map = IndexMap::new();
            for (k, val) in obj {
                map.insert(k.clone(), json_to_node(val));
            }
            Node::Object(map)
        }
        _ => panic!("Unknown type: {}", ty),
    }
}

/// Compare two nodes, handling NaN specially.
fn assert_node_eq(actual: &Node, expected: &Node, context: &str) {
    match (actual, expected) {
        (Node::Float(a), Node::Float(e)) => {
            if e.is_nan() {
                assert!(a.is_nan(), "{}: expected NaN, got {}", context, a);
            } else if *e == 0.0 && e.is_sign_negative() {
                assert_eq!(*a, 0.0, "{}: expected -0.0", context);
                assert!(a.is_sign_negative(), "{}: expected negative zero", context);
            } else {
                assert!(
                    (a - e).abs() < 1e-10 || a == e,
                    "{}: expected {}, got {}",
                    context,
                    e,
                    a
                );
            }
        }
        (Node::List(a_items), Node::List(e_items)) => {
            assert_eq!(
                a_items.len(),
                e_items.len(),
                "{}: list length mismatch",
                context
            );
            for (i, (a, e)) in a_items.iter().zip(e_items.iter()).enumerate() {
                assert_node_eq(a, e, &format!("{}[{}]", context, i));
            }
        }
        (Node::Object(a_map), Node::Object(e_map)) => {
            let actual_keys: Vec<_> = a_map.keys().collect();
            let expected_keys: Vec<_> = e_map.keys().collect();
            assert_eq!(
                actual_keys, expected_keys,
                "{}: object key order mismatch",
                context,
            );
            for ((a_key, a_val), (e_key, e_val)) in a_map.iter().zip(e_map.iter()) {
                assert_eq!(a_key, e_key, "{}: object key mismatch", context);
                assert_node_eq(a_val, e_val, &format!("{}.{}", context, e_key));
            }
        }
        _ => assert_eq!(actual, expected, "{}", context),
    }
}

fn should_skip(test: &Value) -> bool {
    if let Some(skip) = test.get("skip").and_then(|s| s.as_array()) {
        return skip.iter().any(|s| s.as_str() == Some("rust"));
    }
    false
}

fn parse_mode_from_test(test: &Value) -> ParseMode {
    match test.get("mode").and_then(|v| v.as_str()) {
        Some("strict") => ParseMode::Strict,
        Some("compat") | None => ParseMode::Compat,
        Some(other) => panic!("Unknown fixture mode: {}", other),
    }
}

fn parse_mode_override(test: &Value) -> Option<ParseMode> {
    test.get("mode").map(|_| parse_mode_from_test(test))
}

fn parse_with_mode(input: &str, mode: ParseMode) -> super::Result<Node> {
    crate::from_ttoon_with_mode(input, mode)
}

fn format_from_str(s: &str) -> Format {
    match s {
        "tjson" => Format::Tjson,
        "ttoon" => Format::Ttoon,
        "typed_unit" => Format::TypedUnit,
        _ => panic!("Unknown format: {}", s),
    }
}

struct FixtureOpts {
    format: Option<Format>,
    ttoon: TtoonOptions,
    tjson: TjsonOptions,
}

/// Extract format and per-format options from fixture test options JSON.
fn make_fixture_opts(opts: &Value) -> Result<FixtureOpts> {
    let mut binary_format = BinaryFormat::Hex;
    let mut indent_size = 2u8;
    let mut delimiter = Delimiter::Comma;
    let mut format = None;

    if let Some(obj) = opts.as_object() {
        if let Some(v) = obj.get("binary_format").and_then(|v| v.as_str()) {
            binary_format = BinaryFormat::parse(v).ok_or_else(|| {
                Error::new(
                    ErrorKind::SerializeError,
                    format!("unknown binary_format '{}'", v),
                    None,
                )
            })?;
        }
        if let Some(v) = obj.get("indent_size").and_then(|v| v.as_u64()) {
            indent_size = v as u8;
        }
        if let Some(v) = obj.get("delimiter").and_then(|v| v.as_str()) {
            delimiter = Delimiter::parse(v).ok_or_else(|| {
                Error::new(
                    ErrorKind::SerializeError,
                    format!("unknown delimiter '{}'", v),
                    None,
                )
            })?;
        }
        if let Some(v) = obj.get("format").and_then(|v| v.as_str()) {
            format = Some(format_from_str(v));
        }
    }

    Ok(FixtureOpts {
        format,
        ttoon: TtoonOptions {
            binary_format,
            indent_size,
            delimiter,
        },
        tjson: TjsonOptions { binary_format },
    })
}

fn assert_arrow_field_matches(field: &arrow_schema::Field, spec: &Value, id: &str, col_name: &str) {
    use arrow_schema::{DataType, TimeUnit};

    if let Some(type_str) = spec.as_str() {
        let (expected_dt, nullable) = match type_str {
            "int" => (DataType::Int64, false),
            "int_nullable" => (DataType::Int64, true),
            "float" => (DataType::Float64, false),
            "bool" => (DataType::Boolean, false),
            "string" => (DataType::Utf8, false),
            _ => panic!("[{}] unknown legacy read_arrow type: {}", id, type_str),
        };
        assert_eq!(
            field.data_type(),
            &expected_dt,
            "[{}] field '{}' type mismatch",
            id,
            col_name
        );
        if nullable {
            assert!(
                field.is_nullable(),
                "[{}] field '{}' should be nullable",
                id,
                col_name
            );
        }
        return;
    }

    let obj = spec.as_object().unwrap_or_else(|| {
        panic!(
            "[{}] field '{}' schema spec must be string or object",
            id, col_name
        )
    });
    let ty = obj
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("[{}] field '{}' schema spec missing type", id, col_name));
    let expected_nullable = obj.get("nullable").and_then(|v| v.as_bool());

    let expected_dt = match ty {
        "null" => DataType::Null,
        "bool" => DataType::Boolean,
        "int64" => DataType::Int64,
        "float64" => DataType::Float64,
        "utf8" => DataType::Utf8,
        "binary" => DataType::Binary,
        "uuid" => DataType::FixedSizeBinary(16),
        "date32" => DataType::Date32,
        "time64" => {
            let unit = obj
                .get("unit")
                .and_then(|v| v.as_str())
                .unwrap_or("microsecond");
            match unit {
                "microsecond" => DataType::Time64(TimeUnit::Microsecond),
                _ => panic!("[{}] unsupported time64 unit '{}'", id, unit),
            }
        }
        "timestamp" => {
            let unit = obj
                .get("unit")
                .and_then(|v| v.as_str())
                .unwrap_or("microsecond");
            let timezone = obj.get("timezone").and_then(|v| v.as_str());
            match (unit, timezone) {
                ("microsecond", Some(tz)) => {
                    DataType::Timestamp(TimeUnit::Microsecond, Some(Arc::<str>::from(tz)))
                }
                ("microsecond", None) => DataType::Timestamp(TimeUnit::Microsecond, None),
                _ => panic!("[{}] unsupported timestamp unit '{}'", id, unit),
            }
        }
        "decimal128" => {
            let precision = obj
                .get("precision")
                .and_then(|v| v.as_u64())
                .unwrap_or_else(|| panic!("[{}] decimal128 missing precision", id))
                as u8;
            let scale = obj
                .get("scale")
                .and_then(|v| v.as_i64())
                .unwrap_or_else(|| panic!("[{}] decimal128 missing scale", id))
                as i8;
            DataType::Decimal128(precision, scale)
        }
        "decimal256" => {
            let precision = obj
                .get("precision")
                .and_then(|v| v.as_u64())
                .unwrap_or_else(|| panic!("[{}] decimal256 missing precision", id))
                as u8;
            let scale = obj
                .get("scale")
                .and_then(|v| v.as_i64())
                .unwrap_or_else(|| panic!("[{}] decimal256 missing scale", id))
                as i8;
            DataType::Decimal256(precision, scale)
        }
        other => panic!("[{}] unknown schema type '{}'", id, other),
    };

    assert_eq!(
        field.data_type(),
        &expected_dt,
        "[{}] field '{}' type mismatch",
        id,
        col_name
    );

    if ty == "uuid" {
        assert_eq!(
            field
                .metadata()
                .get("ARROW:extension:name")
                .map(|v| v.as_str()),
            Some("arrow.uuid"),
            "[{}] field '{}' uuid metadata mismatch",
            id,
            col_name
        );
    }

    if let Some(nullable) = expected_nullable {
        assert_eq!(
            field.is_nullable(),
            nullable,
            "[{}] field '{}' nullable mismatch",
            id,
            col_name
        );
    }
}

fn fixture_error_kind(tag: &str) -> ErrorKind {
    match tag {
        "lex_error" => ErrorKind::LexError,
        "parse_error" => ErrorKind::ParseError,
        "arrow_error" => ErrorKind::ArrowError,
        "serialize_error" => ErrorKind::SerializeError,
        "transcode_error" => ErrorKind::TranscodeError,
        other => panic!("unknown fixture error kind: {}", other),
    }
}

// ─── Test Functions ─────────────────────────────────────────────────────────────

#[test]
fn test_fixture_format_detect() {
    let fixture = load_fixture("format_detect.json");
    let tests = fixture["tests"].as_array().unwrap();

    for test in tests {
        if should_skip(test) {
            continue;
        }
        let id = test["id"].as_str().unwrap();
        let input = test["input"].as_str().unwrap();
        let expected = test["expected_format"].as_str().unwrap();

        let format = format_detect::detect(input);
        let format_str = match format {
            Format::Tjson => "tjson",
            Format::Ttoon => "ttoon",
            Format::TypedUnit => "typed_unit",
        };
        assert_eq!(format_str, expected, "[{}] format mismatch", id);

        // 若 fixture 標記 expected_parse_error，驗證解析確實失敗
        // （邊界 case：format_detect 正確路由，parser 拒絕不合法的語法）
        if test.get("expected_parse_error").and_then(|v| v.as_bool()) == Some(true) {
            let result = crate::from_ttoon(input);
            assert!(result.is_err(), "[{}] expected parse error but got Ok", id);
            if let Some(needle) = test
                .get("expected_parse_error_contains")
                .and_then(|v| v.as_str())
            {
                let err = result.unwrap_err();
                assert!(
                    err.message.contains(needle),
                    "[{}] expected parse error containing {:?}, got {:?}",
                    id,
                    needle,
                    err.message
                );
            }
        }
    }
}

#[test]
fn test_fixture_parse_scalars() {
    let fixture = load_fixture("parse_scalars.json");
    let tests = fixture["tests"].as_array().unwrap();

    for test in tests {
        if should_skip(test) {
            continue;
        }
        let id = test["id"].as_str().unwrap();
        let mode = parse_mode_from_test(test);

        if let Some(input) = test.get("input").and_then(|v| v.as_str()) {
            let direct_parser = test.get("direct_parser").and_then(|v| v.as_str());
            let parse_fn = |s: &str| -> super::Result<Node> {
                match direct_parser {
                    Some("parse_value") => parser::parse_value_with_mode(s, mode),
                    _ => parser::parse_structure(s),
                }
            };
            if let Some(expected) = test.get("expected") {
                if expected.get("error").is_some() {
                    let result = parse_fn(input);
                    assert!(result.is_err(), "[{}] expected error", id);
                } else {
                    let result = parse_fn(input);
                    assert!(result.is_ok(), "[{}] parse failed: {:?}", id, result);
                    let node = result.unwrap();
                    let expected_node = json_to_node(expected);
                    assert_node_eq(&node, &expected_node, id);
                }
            }
            if let Some(expected_output) = test.get("expected_output").and_then(|v| v.as_str()) {
                let node = parse_fn(input).unwrap();
                let output = serializer::serialize_tjson(&node, &TjsonOptions::default()).unwrap();
                assert_eq!(output, expected_output, "[{}] serialize mismatch", id);
            }
        }
    }
}

#[test]
fn test_fixture_parse_integers() {
    let fixture = load_fixture("parse_integers.json");
    run_parse_tests(&fixture);
}

#[test]
fn test_fixture_parse_floats() {
    let fixture = load_fixture("parse_floats.json");
    let tests = fixture["tests"].as_array().unwrap();

    for test in tests {
        if should_skip(test) {
            continue;
        }
        let id = test["id"].as_str().unwrap();

        if let Some(input) = test.get("input").and_then(|v| v.as_str()) {
            if let Some(expected) = test.get("expected") {
                if expected.get("error").is_some() {
                    let result = parser::parse_structure(input);
                    assert!(result.is_err(), "[{}] expected error", id);
                } else {
                    let result = parser::parse_structure(input);
                    assert!(result.is_ok(), "[{}] parse failed: {:?}", id, result);
                    let node = result.unwrap();
                    let expected_node = json_to_node(expected);
                    assert_node_eq(&node, &expected_node, id);
                }
            }
            if let Some(contains) = test
                .get("expected_output_contains")
                .and_then(|v| v.as_array())
            {
                let node = parser::parse_structure(input).unwrap();
                let output = serializer::serialize_tjson(&node, &TjsonOptions::default()).unwrap();
                for s in contains {
                    let s = s.as_str().unwrap();
                    assert!(
                        output.contains(s),
                        "[{}] expected output to contain '{}', got '{}'",
                        id,
                        s,
                        output
                    );
                }
            }
        }
    }
}

#[test]
fn test_fixture_parse_strings() {
    let fixture = load_fixture("parse_strings.json");
    run_parse_tests(&fixture);
}

#[test]
fn test_fixture_parse_typed_cells() {
    let fixture = load_fixture("parse_typed_cells.json");
    run_parse_tests(&fixture);
}

#[test]
fn test_fixture_parse_date_time() {
    let fixture = load_fixture("parse_date_time.json");
    run_parse_tests(&fixture);
}

#[test]
fn test_fixture_parse_structures() {
    let fixture = load_fixture("parse_structures.json");
    run_parse_tests(&fixture);
}

#[test]
fn test_fixture_parse_ttoon_structure() {
    let fixture = load_fixture("parse_ttoon_structure.json");
    let tests = fixture["tests"].as_array().unwrap();

    for test in tests {
        if should_skip(test) {
            continue;
        }
        let id = test["id"].as_str().unwrap();
        let input = test["input"].as_str().unwrap();
        let use_direct_parser = test.get("parser").and_then(|v| v.as_str()) == Some("ttoon");
        let mode = parse_mode_from_test(test);

        if let Some(expected) = test.get("expected") {
            if expected.get("error").is_some() {
                let result = if use_direct_parser {
                    ttoon_parser::parse_ttoon_structure(input, mode)
                } else {
                    parse_with_mode(input, mode)
                };
                assert!(result.is_err(), "[{}] expected error", id);
            } else {
                let result = if use_direct_parser {
                    ttoon_parser::parse_ttoon_structure(input, mode)
                } else {
                    parse_with_mode(input, mode)
                };
                assert!(result.is_ok(), "[{}] parse failed: {:?}", id, result);
                let node = result.unwrap();
                let expected_node = json_to_node(expected);
                assert_node_eq(&node, &expected_node, id);
            }
        }
    }
}

fn run_parse_ttoon_tabular_fixture(fixture_name: &str) {
    let fixture = load_fixture(fixture_name);
    let tests = fixture["tests"].as_array().unwrap();

    for test in tests {
        if should_skip(test) {
            continue;
        }
        let id = test["id"].as_str().unwrap();
        let input = test["input"].as_str().unwrap();
        let direct_parser = test.get("parser").and_then(|v| v.as_str());
        let mode = parse_mode_from_test(test);

        if let Some(expected) = test.get("expected") {
            let result = match direct_parser {
                Some("ttoon") => ttoon_parser::parse_ttoon_structure(input, mode),
                _ => parse_with_mode(input, mode),
            };

            if expected.get("error").is_some() {
                assert!(result.is_err(), "[{}] expected error", id);
            } else {
                assert!(result.is_ok(), "[{}] parse failed: {:?}", id, result);
                let node = result.unwrap();
                let expected_node = json_to_node(expected);
                assert_node_eq(&node, &expected_node, id);
            }
        }
    }
}

#[test]
fn test_fixture_parse_ttoon_tabular_exact() {
    run_parse_ttoon_tabular_fixture("parse_ttoon_tabular_exact.json");
}

#[test]
fn test_fixture_parse_ttoon_tabular_streaming() {
    run_parse_ttoon_tabular_fixture("parse_ttoon_tabular_streaming.json");
}

#[test]
fn test_fixture_validation_errors() {
    // 驗證即解析（R18）：型別驗證已融入解析階段，
    // 以下測試案例應在 parse 時直接返回錯誤（ParseError）。
    let fixture = load_fixture("validation_errors.json");
    let tests = fixture["tests"].as_array().unwrap();

    for test in tests {
        if should_skip(test) {
            continue;
        }
        let id = test["id"].as_str().unwrap();
        let input = test["input"].as_str().unwrap();
        let mode_override = parse_mode_override(test);

        let result = match mode_override {
            Some(mode) => parse_with_mode(input, mode),
            None => parser::parse_structure(input),
        };
        assert!(
            result.is_err(),
            "[{}] expected parse error but succeeded",
            id
        );
        assert!(
            matches!(result, Err(ref err) if err.kind == ErrorKind::ParseError),
            "[{}] expected ParseError, got {:?}",
            id,
            result
        );
    }
}

#[test]
fn test_fixture_error_lex() {
    let fixture = load_fixture("error_lex.json");
    let tests = fixture["tests"].as_array().unwrap();

    for test in tests {
        if should_skip(test) {
            continue;
        }
        let id = test["id"].as_str().unwrap();
        let input = test["input"].as_str().unwrap();
        let mode = parse_mode_from_test(test);

        let result = match mode {
            ParseMode::Compat => parser::parse_structure(input),
            ParseMode::Strict => parse_with_mode(input, mode),
        };
        assert!(
            matches!(result, Err(ref err) if err.kind == ErrorKind::LexError),
            "[{}] expected LexError, got {:?}",
            id,
            result
        );
    }
}

#[test]
fn test_fixture_roundtrip() {
    let fixture = load_fixture("roundtrip.json");
    let tests = fixture["tests"].as_array().unwrap();

    for test in tests {
        if should_skip(test) {
            continue;
        }
        let id = test["id"].as_str().unwrap();
        let value_node = json_to_node(&test["value"]);
        let format_str = test
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("tjson");
        let format = format_from_str(format_str);

        // Wrap in List for tjson format (parse_value auto-detects)
        let node = match format {
            Format::Tjson => Node::List(vec![value_node.clone()]),
            Format::Ttoon | Format::TypedUnit => {
                // For T-TOON, wrap uuid/binary in a List to serialize
                match &value_node {
                    Node::Uuid(_) | Node::Binary(_) => Node::List(vec![value_node.clone()]),
                    _ => value_node.clone(),
                }
            }
        };

        let fixture_opts = test
            .get("options")
            .map(make_fixture_opts)
            .transpose()
            .unwrap();
        let serialized = match format {
            Format::Tjson => {
                let tjson_opts = fixture_opts
                    .as_ref()
                    .map(|o| o.tjson.clone())
                    .unwrap_or_default();
                serializer::serialize_tjson(&node, &tjson_opts).unwrap()
            }
            Format::Ttoon | Format::TypedUnit => {
                let ttoon_opts = fixture_opts.as_ref().map(|o| o.ttoon.clone());
                crate::to_ttoon(&node, ttoon_opts.as_ref()).unwrap()
            }
        };
        let deserialized = parser::parse_value(&serialized).unwrap();

        assert_node_eq(&deserialized, &node, id);
    }
}

fn assert_serialize_output(test: &Value, output: &str, id: &str) {
    if let Some(expected_output) = test.get("expected_output").and_then(|v| v.as_str()) {
        assert_eq!(
            output.trim_end(),
            expected_output.trim_end(),
            "[{}] output mismatch",
            id
        );
    }
    if let Some(contains) = test
        .get("expected_output_contains")
        .and_then(|v| v.as_array())
    {
        for s in contains {
            let s = s.as_str().unwrap();
            assert!(
                output.contains(s),
                "[{}] expected output to contain '{}', got: {}",
                id,
                s,
                output
            );
        }
    }
    if let Some(not_contains) = test
        .get("expected_output_not_contains")
        .and_then(|v| v.as_array())
    {
        for s in not_contains {
            let s = s.as_str().unwrap();
            assert!(
                !output.contains(s),
                "[{}] expected output NOT to contain '{}', got: {}",
                id,
                s,
                output
            );
        }
    }
    if let Some(starts_with) = test
        .get("expected_output_starts_with")
        .and_then(|v| v.as_str())
    {
        assert!(
            output.starts_with(starts_with),
            "[{}] expected output to start with '{}', got: {}",
            id,
            starts_with,
            output
        );
    }
}

fn run_serialize_batch_fixture(fixture_name: &str) {
    let fixture = load_fixture(fixture_name);
    let tests = fixture["tests"].as_array().unwrap();

    for test in tests {
        if should_skip(test) {
            continue;
        }
        let id = test["id"].as_str().unwrap();
        let value_node = json_to_node(&test["value"]);

        let options_result = if let Some(opts_json) = test.get("options") {
            make_fixture_opts(opts_json).map(Some)
        } else {
            Ok(None)
        };

        // Check error case
        if let Some(expected) = test.get("expected") {
            if let Some(err_kind) = expected.get("error").and_then(|e| e.as_str()) {
                if let Err(err) = options_result.as_ref() {
                    assert_eq!(err.kind, ErrorKind::SerializeError, "[{}]", id);
                    continue;
                }
                let fixture_opts = options_result.as_ref().unwrap();
                let fmt = fixture_opts
                    .as_ref()
                    .and_then(|o| o.format)
                    .unwrap_or(Format::Tjson);
                let result = match fmt {
                    Format::Tjson => {
                        let tjson_opts = fixture_opts
                            .as_ref()
                            .map(|o| o.tjson.clone())
                            .unwrap_or_default();
                        serializer::serialize_tjson(&value_node, &tjson_opts)
                    }
                    Format::Ttoon | Format::TypedUnit => {
                        let ttoon_opts = fixture_opts.as_ref().map(|o| o.ttoon.clone());
                        crate::to_ttoon(&value_node, ttoon_opts.as_ref())
                    }
                };
                assert!(result.is_err(), "[{}] expected error", id);
                match err_kind {
                    "serialize_error" => {
                        assert_eq!(
                            result.unwrap_err().kind,
                            ErrorKind::SerializeError,
                            "[{}]",
                            id
                        );
                    }
                    _ => {}
                }
                continue;
            }
        }

        let fixture_opts = options_result.unwrap();
        let fmt = fixture_opts
            .as_ref()
            .and_then(|o| o.format)
            .unwrap_or(Format::Tjson);

        let result = match fmt {
            Format::Tjson => {
                let tjson_opts = fixture_opts
                    .as_ref()
                    .map(|o| o.tjson.clone())
                    .unwrap_or_default();
                serializer::serialize_tjson(&value_node, &tjson_opts)
            }
            Format::Ttoon | Format::TypedUnit => {
                let ttoon_opts = fixture_opts.as_ref().map(|o| o.ttoon.clone());
                crate::to_ttoon(&value_node, ttoon_opts.as_ref())
            }
        };
        assert!(result.is_ok(), "[{}] serialize failed: {:?}", id, result);
        let output = result.unwrap();
        assert_serialize_output(test, &output, id);
    }
}

#[test]
fn test_fixture_serialize_options() {
    run_serialize_batch_fixture("serialize_options.json");
}

#[test]
fn test_fixture_serialize_ttoon_tabular_exact() {
    run_serialize_batch_fixture("serialize_ttoon_tabular_exact.json");
}

#[test]
fn test_fixture_serialize_ttoon_tabular_streaming() {
    use super::streaming::StreamWriter;
    use super::{FieldType, ScalarType, StreamSchema};

    let fixture = load_fixture("serialize_ttoon_tabular_streaming.json");
    let tests = fixture["tests"].as_array().unwrap();

    for test in tests {
        if should_skip(test) {
            continue;
        }
        let id = test["id"].as_str().unwrap();
        let fields: Vec<&str> = test["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let rows_json = test["rows"].as_array().unwrap();

        // Build rows as IndexMap<String, Node>
        let rows: Vec<IndexMap<String, Node>> = rows_json
            .iter()
            .map(|row_obj| {
                let obj = row_obj.as_object().unwrap();
                let mut map = IndexMap::new();
                for &f in &fields {
                    if let Some(val) = obj.get(f) {
                        map.insert(f.to_string(), json_to_node(val));
                    }
                }
                map
            })
            .collect();

        // Infer schema from fields + all rows
        let schema_fields: Vec<(&str, FieldType)> = fields
            .iter()
            .map(|&f| {
                let mut nullable = false;
                let mut scalar = ScalarType::String; // default
                let mut found_type = false;
                for row in &rows {
                    if let Some(node) = row.get(f) {
                        match node {
                            Node::Null => {
                                nullable = true;
                            }
                            Node::String(_) if !found_type => {
                                scalar = ScalarType::String;
                                found_type = true;
                            }
                            Node::Int(_) if !found_type => {
                                scalar = ScalarType::Int;
                                found_type = true;
                            }
                            Node::Float(_) if !found_type => {
                                scalar = ScalarType::Float;
                                found_type = true;
                            }
                            Node::Bool(_) if !found_type => {
                                scalar = ScalarType::Bool;
                                found_type = true;
                            }
                            _ => {}
                        }
                    }
                }
                if nullable {
                    (f, FieldType::nullable(scalar))
                } else {
                    (f, FieldType::new(scalar))
                }
            })
            .collect();
        let schema = StreamSchema::try_new(schema_fields).unwrap();

        // Write using StreamWriter
        let mut output_buf = Vec::new();
        let mut writer = StreamWriter::new(&mut output_buf, schema, TtoonOptions::default());
        for row in &rows {
            writer.write(row).unwrap();
        }
        writer.close().unwrap();
        let output = String::from_utf8(output_buf).unwrap();

        assert_serialize_output(test, &output, id);
    }
}

#[test]
fn test_fixture_read_arrow() {
    let fixture = load_fixture("read_arrow.json");
    let tests = fixture["tests"].as_array().unwrap();

    for test in tests {
        if should_skip(test) {
            continue;
        }
        let id = test["id"].as_str().unwrap();
        let input = test["input"].as_str().unwrap();

        if let Some(expected) = test.get("expected") {
            if let Some(error_tag) = expected.get("error").and_then(|v| v.as_str()) {
                let result = crate::read_arrow(input);
                assert!(
                    result.is_err(),
                    "[{}] expected {} but succeeded",
                    id,
                    error_tag
                );
                assert_eq!(
                    result.unwrap_err().kind,
                    fixture_error_kind(error_tag),
                    "[{}] expected {} kind",
                    id,
                    error_tag
                );
                continue;
            }
        }

        let result = crate::read_arrow(input);
        assert!(result.is_ok(), "[{}] read_arrow failed: {:?}", id, result);
        let table = result.unwrap();

        if let Some(num_rows) = test.get("expected_num_rows").and_then(|v| v.as_u64()) {
            assert_eq!(
                table.num_rows(),
                num_rows as usize,
                "[{}] num_rows mismatch",
                id
            );
        }
        if let Some(num_cols) = test.get("expected_num_cols").and_then(|v| v.as_u64()) {
            assert_eq!(
                table.schema.fields().len(),
                num_cols as usize,
                "[{}] num_cols mismatch",
                id
            );
        }
        if let Some(expected_field_order) =
            test.get("expected_field_order").and_then(|v| v.as_array())
        {
            let actual_field_order: Vec<&str> = table
                .schema
                .fields()
                .iter()
                .map(|field| field.name().as_str())
                .collect();
            let expected_field_order: Vec<&str> = expected_field_order
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .unwrap_or_else(|| panic!("[{}] expected_field_order must be string", id))
                })
                .collect();
            assert_eq!(
                actual_field_order, expected_field_order,
                "[{}] field order mismatch",
                id
            );
        }
        if let Some(schema_map) = test.get("expected_schema").and_then(|v| v.as_object()) {
            for (col_name, spec) in schema_map {
                let field = table
                    .schema
                    .field_with_name(col_name)
                    .unwrap_or_else(|_| panic!("[{}] missing field '{}'", id, col_name));
                assert_arrow_field_matches(field.as_ref(), spec, id, col_name);
            }
        }
        if let Some(expected_rows) = test.get("expected_rows").and_then(|v| v.as_array()) {
            let actual_rows = crate::arrow_to_tjson(&table, None)
                .unwrap_or_else(|e| panic!("[{}] arrow_to_tjson failed: {:?}", id, e));
            let actual_rows = parser::parse_structure(&actual_rows)
                .unwrap_or_else(|e| panic!("[{}] parse actual rows failed: {:?}", id, e));
            let expected_rows = Node::List(expected_rows.iter().map(json_to_node).collect());
            assert_node_eq(
                &actual_rows,
                &expected_rows,
                &format!("[{}].expected_rows", id),
            );
        }
    }
}

// ─── Common parse test runner ───────────────────────────────────────────────────

fn run_parse_tests(fixture: &Value) {
    let tests = fixture["tests"].as_array().unwrap();

    for test in tests {
        if should_skip(test) {
            continue;
        }
        let id = test["id"].as_str().unwrap();
        let mode = parse_mode_from_test(test);

        if let Some(input) = test.get("input").and_then(|v| v.as_str()) {
            if let Some(expected) = test.get("expected") {
                if expected.get("error").is_some() {
                    let result = parse_with_mode(input, mode);
                    assert!(
                        result.is_err(),
                        "[{}] expected error for input: {}",
                        id,
                        input
                    );
                } else {
                    let result = parse_with_mode(input, mode);
                    assert!(result.is_ok(), "[{}] parse failed: {:?}", id, result);
                    let node = result.unwrap();
                    let expected_node = json_to_node(expected);
                    assert_node_eq(&node, &expected_node, id);
                }
            }
        }
    }
}
