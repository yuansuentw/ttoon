// Shared / data-driven test cases live in `tests/fixtures/*.json` and are
// executed by `fixture_tests.rs`.  The tests below cover behaviour that is
// unique to the Rust implementation or not expressible in the fixture format.

use super::ir::Node;
use super::tjson_parser as parser;
use super::tjson_serializer as serializer;
use super::ttoon_serializer;
use super::typed_parse::ParseMode;
use super::{
    BinaryFormat, Delimiter, ErrorKind, TjsonOptions, TranscodeOperation, TranscodePhase,
    TtoonOptions,
};

#[test]
fn test_parse_value_route_with_structure() {
    let node = parser::parse_value("[1, 2]").unwrap();
    assert_eq!(node, Node::List(vec![Node::Int(1), Node::Int(2)]));
}

#[test]
fn test_float_precision_edge_cases() {
    // 測試精度保留
    let precise_values = vec![
        0.1,
        0.2,
        0.3,
        1.1,
        1.7976931348623157e308,  // 接近 f64::MAX
        2.2250738585072014e-308, // 接近 f64::MIN_POSITIVE
    ];

    for value in precise_values {
        let node = Node::Float(value);
        let serialized = serializer::serialize_tjson(&node, &TjsonOptions::default()).unwrap();
        let deserialized = parser::parse_value(&serialized).unwrap();

        if let Node::Float(result) = deserialized {
            assert_eq!(value, result, "precision lost for: {}", value);
        }
    }
}

#[test]
fn test_structure_deep_nesting() {
    // 10 層嵌套
    let deep_list = "[[[[[[[[[[]]]]]]]]]]";
    let node = parser::parse_structure(deep_list).unwrap();

    // 驗證深度 - 從最外層開始計數
    fn count_depth(node: &Node) -> usize {
        match node {
            Node::List(items) if items.len() == 1 => 1 + count_depth(&items[0]),
            Node::List(items) if items.is_empty() => 1, // 空 list 也算 1 層
            _ => 0,
        }
    }

    // 10 個開括號 = 10 層
    assert_eq!(count_depth(&node), 10);
}

#[test]
fn test_structure_wide_list() {
    // 100 個元素
    let items: Vec<String> = (0..100).map(|i| i.to_string()).collect();
    let input = format!("[{}]", items.join(", "));

    let node = parser::parse_structure(&input).unwrap();
    if let Node::List(parsed_items) = node {
        assert_eq!(parsed_items.len(), 100);
    }
}

#[test]
fn test_nested_structures_complex() {
    let input = r#"{
        "users": [
            {"id": 1, "name": "Alice", "scores": [95, 87, 92]},
            {"id": 2, "name": "Bob", "scores": [88, 91, 85]}
        ],
        "metadata": {
            "count": 2,
            "updated": "2024-01-01T00:00:00Z"
        }
    }"#;

    let node = parser::parse_structure(input).unwrap();

    // 往返測試
    let serialized = serializer::serialize_tjson(&node, &TjsonOptions::default()).unwrap();
    let reparsed = parser::parse_structure(&serialized).unwrap();
    assert_eq!(node, reparsed);
}

#[test]
fn test_error_span_information() {
    // 測試錯誤訊息包含位置資訊
    let result = parser::parse_structure("[1, 2, 03]");

    assert!(result.is_err());
    if let Err(err) = result {
        assert!(err.span.is_some(), "error should have span information");
        // span 應該指向錯誤位置
    }
}

#[test]
fn test_lexer_valid_unicode_escape() {
    let cases = vec![
        (r#"["\u0041"]"#, "A"),        // U+0041 = 'A'
        (r#"["\u4E2D"]"#, "中"),       // U+4E2D = '中'
        (r#"["\u0000"]"#, "\u{0000}"), // NULL character
    ];

    for (input, expected) in cases {
        let node = parser::parse_structure(input).unwrap();
        assert_eq!(
            node,
            Node::List(vec![Node::String(expected.to_string())]),
            "failed for: {}",
            input
        );
    }
}

#[test]
fn test_tjson_quoted_string_decode_uses_json_authority() {
    let err = parser::parse_structure(r#"["\uD83D"]"#).unwrap_err();
    assert_eq!(err.kind, ErrorKind::LexError);

    let node = parser::parse_structure(r#"["\uD83D\uDE00"]"#).unwrap();
    assert_eq!(node, Node::List(vec![Node::String("😀".to_string())]));
}

#[test]
fn test_parse_mode_defaults_and_tjson_strict() {
    let compat = crate::ttoon_parser::parse_ttoon("key: hello", ParseMode::Compat).unwrap();
    assert_eq!(
        compat,
        Node::Object({
            let mut m = indexmap::IndexMap::new();
            m.insert("key".to_string(), Node::String("hello".to_string()));
            m
        })
    );

    let strict = crate::ttoon_parser::parse_ttoon("key: hello", ParseMode::Strict);
    assert!(strict.is_err());

    let bare_compat = crate::from_ttoon("@").unwrap();
    assert_eq!(bare_compat, Node::String("@".to_string()));

    let bare_strict = crate::from_ttoon_with_mode("@", ParseMode::Strict);
    assert!(matches!(bare_strict, Err(err) if err.kind == ErrorKind::LexError));

    let tjson = parser::parse_structure(r#"{"key": foo}"#);
    assert!(tjson.is_err());
}

#[test]
fn test_empty_binary_b64_roundtrip() {
    let node = Node::Binary(Vec::new());
    let opts = TjsonOptions {
        binary_format: BinaryFormat::B64,
    };
    let serialized = serializer::serialize_tjson(&node, &opts).unwrap();
    assert_eq!(serialized, "b64()");
    let reparsed = parser::parse_value(&serialized).unwrap();
    assert_eq!(reparsed, Node::Binary(Vec::new()));
}

// R11: from_ttoon — 公開反序列化 API
#[test]
fn test_from_ttoon_auto_detect() {
    // T-JSON 格式
    let node = crate::from_ttoon(r#"{"key": 42}"#).unwrap();
    assert_eq!(
        node,
        Node::Object({
            let mut m = indexmap::IndexMap::new();
            m.insert("key".to_string(), Node::Int(42));
            m
        })
    );

    // T-TOON 格式
    let node = crate::from_ttoon("key: 42").unwrap();
    assert_eq!(
        node,
        Node::Object({
            let mut m = indexmap::IndexMap::new();
            m.insert("key".to_string(), Node::Int(42));
            m
        })
    );

    // 頂層純量（T-JSON）
    let node = crate::from_ttoon("true").unwrap();
    assert_eq!(node, Node::Bool(true));

    let empty = crate::from_ttoon("").unwrap();
    assert_eq!(empty, Node::Object(indexmap::IndexMap::new()));

    let whitespace = crate::from_ttoon("   ").unwrap();
    assert_eq!(whitespace, Node::Object(indexmap::IndexMap::new()));

    let newline = crate::from_ttoon("\n").unwrap();
    assert_eq!(newline, Node::Object(indexmap::IndexMap::new()));

    // T-TOON tabular
    let node = crate::from_ttoon("[2]{name,score}:\nAlice, 95\nBob, 87").unwrap();
    if let Node::List(rows) = node {
        assert_eq!(rows.len(), 2);
    } else {
        panic!("expected List");
    }
}

#[test]
fn test_detect_format_accepts_streaming_tabular_header() {
    assert_eq!(
        crate::detect_format("[*]{name,age}:\n\"Alice\", 30\n"),
        crate::format_detect::Format::Ttoon
    );
    assert_eq!(
        crate::detect_format("[*]: 1, 2"),
        crate::format_detect::Format::Tjson
    );
}

#[test]
fn test_root_tabular_exact_count_rejects_missing_rows() {
    let err = crate::from_ttoon("[2]{name}:\n\"Alice\"").unwrap_err();
    assert_eq!(err.kind, ErrorKind::ParseError);
    assert!(err.message.contains("tabular row count mismatch"));
}

#[test]
fn test_root_tabular_exact_count_rejects_extra_rows() {
    let err = crate::from_ttoon("[1]{name}:\n\"Alice\"\n\"Bob\"").unwrap_err();
    assert_eq!(err.kind, ErrorKind::ParseError);
    assert!(err.message.contains("tabular row count mismatch"));
}

#[test]
fn test_root_tabular_streaming_header_requires_rows() {
    let err = crate::from_ttoon("[*]{name}:").unwrap_err();
    assert_eq!(err.kind, ErrorKind::ParseError);
    assert!(err.message.contains("[*]"));
}

// R12: to_ttoon — 公開序列化 API
#[test]
fn test_to_ttoon_roundtrip() {
    use indexmap::IndexMap;

    // Object roundtrip
    let node = crate::from_ttoon("name: \"Alice\"\nage: 30").unwrap();
    let serialized = crate::to_ttoon(&node, None).unwrap();
    let reparsed = crate::from_ttoon(&serialized).unwrap();
    assert_eq!(node, reparsed);

    // Tabular list roundtrip（via to_ttoon → from_ttoon）
    let rows = vec![
        {
            let mut m = IndexMap::new();
            m.insert("id".to_string(), Node::Int(1));
            m.insert("val".to_string(), Node::String("x".to_string()));
            Node::Object(m)
        },
        {
            let mut m = IndexMap::new();
            m.insert("id".to_string(), Node::Int(2));
            m.insert("val".to_string(), Node::String("y".to_string()));
            Node::Object(m)
        },
    ];
    let node = Node::List(rows);
    let serialized = crate::to_ttoon(&node, None).unwrap();
    // to_ttoon 應輸出 tabular 格式
    assert!(
        serialized.starts_with("[2]{"),
        "expected tabular, got: {:?}",
        serialized
    );
    let reparsed = crate::from_ttoon(&serialized).unwrap();
    assert_eq!(node, reparsed);

    // pipe delimiter roundtrip
    let opts = TtoonOptions {
        delimiter: Delimiter::Pipe,
        ..Default::default()
    };
    let serialized = crate::to_ttoon(&node, Some(&opts)).unwrap();
    assert!(
        serialized.contains("[2|]"),
        "expected pipe marker in header, got: {:?}",
        serialized
    );
    let reparsed = crate::from_ttoon(&serialized).unwrap();
    assert_eq!(node, reparsed);

    // tab delimiter roundtrip
    let opts = TtoonOptions {
        delimiter: Delimiter::Tab,
        ..Default::default()
    };
    let serialized = crate::to_ttoon(&node, Some(&opts)).unwrap();
    assert!(
        serialized.starts_with("[2\t]"),
        "expected tab marker in header, got: {:?}",
        serialized
    );
    let reparsed = crate::from_ttoon(&serialized).unwrap();
    assert_eq!(node, reparsed);
}

// R13: read_arrow — 公開 Arrow 反序列化 API
#[test]
fn test_read_arrow_basic() {
    use arrow_schema::DataType;

    // T-TOON tabular → ArrowTable
    let table = crate::read_arrow("[3]{name,score}:\nAlice, 95\nBob, 87\nCarol, 92").unwrap();
    assert_eq!(table.num_rows(), 3);
    assert_eq!(table.schema.fields().len(), 2);

    let name_field = table.schema.field_with_name("name").unwrap();
    let score_field = table.schema.field_with_name("score").unwrap();
    assert_eq!(name_field.data_type(), &DataType::Utf8);
    assert_eq!(score_field.data_type(), &DataType::Int64);

    // T-TOON structure（list of objects）→ ArrowTable
    let table = crate::read_arrow("- id: 1\n  active: true\n- id: 2\n  active: false").unwrap();
    assert_eq!(table.num_rows(), 2);

    // T-JSON → ArrowTable
    let table = crate::read_arrow(r#"[{"x": 1.5}, {"x": 2.5}]"#).unwrap();
    assert_eq!(table.num_rows(), 2);
    assert_eq!(
        table.schema.field_with_name("x").unwrap().data_type(),
        &DataType::Float64
    );
}

#[test]
fn test_read_arrow_tjson_preserves_field_order() {
    let table = crate::read_arrow(r#"[{"name": "Alice", "age": 30, "active": true}]"#).unwrap();
    let field_names: Vec<&str> = table
        .schema
        .fields()
        .iter()
        .map(|f| f.name().as_str())
        .collect();
    assert_eq!(field_names, vec!["name", "age", "active"]);
}

#[test]
fn test_read_arrow_errors() {
    // 非 List
    let err = crate::read_arrow(r#"{"key": 1}"#).unwrap_err();
    assert_eq!(err.kind, ErrorKind::ArrowError);

    // List 含非 Object
    let err = crate::read_arrow("[1, 2, 3]").unwrap_err();
    assert_eq!(err.kind, ErrorKind::ArrowError);

    // 嵌套 Object（non-scalar value）
    let err = crate::read_arrow(r#"[{"a": {"nested": 1}}]"#).unwrap_err();
    assert_eq!(err.kind, ErrorKind::ArrowError);
}

// R14: arrow_to_ttoon — ArrowTable → T-TOON tabular
#[test]
fn test_arrow_to_ttoon_roundtrip() {
    use arrow_array::{Int64Array, StringArray};
    use arrow_schema::{DataType, Field, Schema as ArrowSchema};
    use std::sync::Arc;

    let schema = Arc::new(ArrowSchema::new(vec![
        Field::new("name", DataType::Utf8, false),
        Field::new("score", DataType::Int64, false),
    ]));
    let name_arr = StringArray::from(vec!["Alice", "Bob"]);
    let score_arr = Int64Array::from(vec![95_i64, 87]);
    let batch = arrow_array::RecordBatch::try_new(
        schema.clone(),
        vec![Arc::new(name_arr), Arc::new(score_arr)],
    )
    .unwrap();
    let table = super::ir::ArrowTable {
        schema,
        batches: vec![batch],
    };

    let ttoon_str = crate::arrow_to_ttoon(&table, None).unwrap();
    assert!(
        ttoon_str.starts_with("[2]{"),
        "expected tabular header, got: {:?}",
        ttoon_str
    );

    // 可以再解析回來
    let reparsed = crate::read_arrow(&ttoon_str).unwrap();
    assert_eq!(reparsed.num_rows(), 2);
}

// R16: to_tjson + arrow_to_tjson
#[test]
fn test_to_tjson_basic() {
    use indexmap::IndexMap;

    // Object → T-JSON
    let mut m = IndexMap::new();
    m.insert("x".to_string(), Node::Int(1));
    let node = Node::Object(m);
    let s = crate::to_tjson(&node, None).unwrap();
    assert_eq!(s, r#"{"x": 1}"#);

    // List → T-JSON
    let node = Node::List(vec![Node::Int(1), Node::Int(2)]);
    let s = crate::to_tjson(&node, None).unwrap();
    assert_eq!(s, "[1, 2]");
}

#[test]
fn test_arrow_to_tjson_basic() {
    use arrow_array::{Int64Array, StringArray};
    use arrow_schema::{DataType, Field, Schema as ArrowSchema};
    use std::sync::Arc;

    let schema = Arc::new(ArrowSchema::new(vec![
        Field::new("name", DataType::Utf8, false),
        Field::new("score", DataType::Int64, false),
    ]));
    let name_arr = StringArray::from(vec!["Alice", "Bob"]);
    let score_arr = Int64Array::from(vec![95_i64, 87]);
    let batch = arrow_array::RecordBatch::try_new(
        schema.clone(),
        vec![Arc::new(name_arr), Arc::new(score_arr)],
    )
    .unwrap();
    let table = super::ir::ArrowTable {
        schema,
        batches: vec![batch],
    };

    let s = crate::arrow_to_tjson(&table, None).unwrap();
    assert!(s.starts_with('['), "expected JSON array, got: {:?}", s);
    assert!(s.contains("\"name\""), "expected name field");
    assert!(s.contains("\"Alice\""), "expected Alice value");

    // 可以 read_arrow 回來
    let reparsed = crate::read_arrow(&s).unwrap();
    assert_eq!(reparsed.num_rows(), 2);
}

// R10：根層級 tabular list 應序列化為 Tabular 格式（serialize_to_ttoon_structure 路徑）
#[test]
fn test_serialize_root_tabular_via_structure() {
    use indexmap::IndexMap;

    let rows = vec![
        {
            let mut m = IndexMap::new();
            m.insert("name".to_string(), Node::String("Alice".to_string()));
            m.insert("age".to_string(), Node::Int(30));
            Node::Object(m)
        },
        {
            let mut m = IndexMap::new();
            m.insert("name".to_string(), Node::String("Bob".to_string()));
            m.insert("age".to_string(), Node::Int(25));
            Node::Object(m)
        },
    ];
    let node = Node::List(rows);
    let opts = TtoonOptions::default();
    let output = ttoon_serializer::serialize_to_ttoon_structure(&node, &opts).unwrap();

    // 應輸出 tabular header，而非 `- item` 格式
    assert!(
        output.starts_with("[2]{"),
        "expected tabular header, got: {:?}",
        output
    );
    assert!(output.contains("age,name") || output.contains("name,age"));
    assert!(output.contains('"'), "string values should be quoted");

    // 確認可解析回相同結構
    let reparsed = parser::parse_value(&output).unwrap();
    assert_eq!(reparsed, node);
}

#[test]
fn test_structure_stack_limit() {
    // MAX_NESTING is 256.
    // Testing deep nesting to ensure it eventually fails.
    let depth_fail = 300;
    let input_fail = format!("{}[1]{}", "[".repeat(depth_fail), "]".repeat(depth_fail));
    let result_fail = parser::parse_structure(&input_fail);
    assert!(
        matches!(result_fail, Err(err) if err.kind == ErrorKind::ParseError),
        "should fail for depth 300"
    );

    // Verify a reasonable depth passes
    let depth_ok = 100;
    let input_ok = format!("{}[1]{}", "[".repeat(depth_ok), "]".repeat(depth_ok));
    let result_ok = parser::parse_structure(&input_ok);
    assert!(result_ok.is_ok(), "should pass for depth 100");
}

// ─── Direct Transcode Tests ─────────────────────────────────────────────────

#[test]
fn test_tjson_to_ttoon_basic_object() {
    let tjson = r#"{"name": "Alice", "age": 30}"#;
    let ttoon = crate::tjson_to_ttoon(tjson, None).unwrap();
    assert_eq!(ttoon, "name: \"Alice\"\nage: 30\n");
}

#[test]
fn test_tjson_to_ttoon_list() {
    let tjson = "[1, 2, 3]";
    let ttoon = crate::tjson_to_ttoon(tjson, None).unwrap();
    // Scalar list → inline array format [N]: v1, v2, v3
    assert!(ttoon.starts_with("[3]:"), "got: {:?}", ttoon);
}

#[test]
fn test_tjson_to_ttoon_tabular() {
    let tjson = r#"[{"a": 1, "b": 2}, {"a": 3, "b": 4}]"#;
    let ttoon = crate::tjson_to_ttoon(tjson, None).unwrap();
    // List of uniform objects → tabular format
    assert!(
        ttoon.starts_with("[2]{"),
        "expected tabular, got: {:?}",
        ttoon
    );
}

#[test]
fn test_tjson_to_ttoon_scalar() {
    assert_eq!(crate::tjson_to_ttoon("42", None).unwrap(), "42\n");
    assert_eq!(crate::tjson_to_ttoon("true", None).unwrap(), "true\n");
    assert_eq!(crate::tjson_to_ttoon("null", None).unwrap(), "null\n");
    assert_eq!(crate::tjson_to_ttoon("3.14", None).unwrap(), "3.14\n");
}

#[test]
fn test_tjson_to_ttoon_typed_values() {
    // decimal
    assert_eq!(crate::tjson_to_ttoon("3.14m", None).unwrap(), "3.14m\n");
    // date
    assert_eq!(
        crate::tjson_to_ttoon("2026-03-01", None).unwrap(),
        "2026-03-01\n"
    );
    // uuid
    let uuid_input = "uuid(550e8400-e29b-41d4-a716-446655440000)";
    let ttoon = crate::tjson_to_ttoon(uuid_input, None).unwrap();
    assert!(ttoon.contains("uuid(550e8400-e29b-41d4-a716-446655440000)"));
}

#[test]
fn test_tjson_to_ttoon_quoted_string() {
    // JSON quoted string with unicode escape
    let tjson = r#"["\u0041"]"#;
    let ttoon = crate::tjson_to_ttoon(tjson, None).unwrap();
    assert!(
        ttoon.contains("A"),
        "unicode escape should be decoded, got: {:?}",
        ttoon
    );
}

#[test]
fn test_tjson_to_ttoon_empty_input_errors() {
    // Empty string is not valid T-JSON
    let err = crate::tjson_to_ttoon("", None).unwrap_err();
    assert_eq!(err.kind, ErrorKind::TranscodeError);
    let transcode = err
        .transcode
        .as_ref()
        .expect("expected structured transcode error");
    assert_eq!(transcode.operation, TranscodeOperation::TjsonToTtoon);
    assert_eq!(transcode.phase, TranscodePhase::Parse);
    assert_eq!(transcode.source_kind, ErrorKind::ParseError);
}

#[test]
fn test_tjson_to_ttoon_rejects_bare_ttoon() {
    // Bare T-TOON key-value is not valid T-JSON
    let err = crate::tjson_to_ttoon("key: value", None).unwrap_err();
    assert_eq!(err.kind, ErrorKind::TranscodeError);
    let transcode = err
        .transcode
        .as_ref()
        .expect("expected structured transcode error");
    assert_eq!(transcode.operation, TranscodeOperation::TjsonToTtoon);
    assert_eq!(transcode.phase, TranscodePhase::Parse);
    assert_eq!(transcode.source_kind, ErrorKind::ParseError);
}

#[test]
fn test_ttoon_to_tjson_basic_object() {
    let ttoon = "name: \"Alice\"\nage: 30";
    let tjson = crate::ttoon_to_tjson(ttoon, ParseMode::Compat, None).unwrap();
    assert_eq!(tjson, r#"{"name": "Alice", "age": 30}"#);
}

#[test]
fn test_ttoon_to_tjson_tabular() {
    let ttoon = "[2]{name,score}:\nAlice, 95\nBob, 87";
    let tjson = crate::ttoon_to_tjson(ttoon, ParseMode::Compat, None).unwrap();
    assert!(
        tjson.starts_with('['),
        "expected JSON array, got: {:?}",
        tjson
    );
    assert!(tjson.contains("\"name\""), "got: {:?}", tjson);
    assert!(tjson.contains("\"Alice\""), "got: {:?}", tjson);
}

#[test]
fn test_ttoon_to_tjson_scalar() {
    assert_eq!(
        crate::ttoon_to_tjson("42", ParseMode::Compat, None).unwrap(),
        "42"
    );
    assert_eq!(
        crate::ttoon_to_tjson("true", ParseMode::Compat, None).unwrap(),
        "true"
    );
    assert_eq!(
        crate::ttoon_to_tjson("null", ParseMode::Compat, None).unwrap(),
        "null"
    );
}

#[test]
fn test_ttoon_to_tjson_typed_values() {
    // decimal
    assert_eq!(
        crate::ttoon_to_tjson("3.14m", ParseMode::Compat, None).unwrap(),
        "3.14m"
    );
    // date
    assert_eq!(
        crate::ttoon_to_tjson("2026-03-01", ParseMode::Compat, None).unwrap(),
        "2026-03-01"
    );
}

#[test]
fn test_ttoon_to_tjson_empty_input() {
    // Empty T-TOON → empty object
    let tjson = crate::ttoon_to_tjson("", ParseMode::Compat, None).unwrap();
    assert_eq!(tjson, "{}");
}

#[test]
fn test_ttoon_to_tjson_strict_mode() {
    // In strict mode, unknown bare tokens should error
    let err = crate::ttoon_to_tjson("key: hello", ParseMode::Strict, None).unwrap_err();
    assert_eq!(err.kind, ErrorKind::TranscodeError);
    let transcode = err
        .transcode
        .as_ref()
        .expect("expected structured transcode error");
    assert_eq!(transcode.operation, TranscodeOperation::TtoonToTjson);
    assert_eq!(transcode.phase, TranscodePhase::Parse);
    assert_eq!(transcode.source_kind, ErrorKind::LexError);
}

// ─── Roundtrip Idempotency Tests ─────────────────────────────────────────────
// 核心不變式：parse → serialize → parse 後 IR tree 結構相同

#[test]
fn test_roundtrip_tjson_to_ttoon_to_tjson() {
    let original_tjson = r#"{"name": "Alice", "scores": [95, 87, 92], "active": true}"#;

    // tjson → ttoon
    let ttoon = crate::tjson_to_ttoon(original_tjson, None).unwrap();
    // ttoon → tjson
    let round_tjson = crate::ttoon_to_tjson(&ttoon, ParseMode::Compat, None).unwrap();

    // IR-level comparison: parse both and compare
    let ir_original = crate::tjson_parser::parse_structure(original_tjson).unwrap();
    let ir_round = crate::tjson_parser::parse_structure(&round_tjson).unwrap();
    assert_eq!(
        ir_original, ir_round,
        "IR trees should be identical after roundtrip"
    );
}

#[test]
fn test_roundtrip_ttoon_to_tjson_to_ttoon() {
    let original_ttoon = "name: \"Alice\"\nage: 30\nactive: true";

    // ttoon → tjson
    let tjson = crate::ttoon_to_tjson(original_ttoon, ParseMode::Compat, None).unwrap();
    // tjson → ttoon
    let round_ttoon = crate::tjson_to_ttoon(&tjson, None).unwrap();

    // IR-level comparison
    let ir_original = crate::ttoon_parser::parse_ttoon(original_ttoon, ParseMode::Compat).unwrap();
    let ir_round = crate::ttoon_parser::parse_ttoon(&round_ttoon, ParseMode::Compat).unwrap();
    assert_eq!(
        ir_original, ir_round,
        "IR trees should be identical after roundtrip"
    );
}

#[test]
fn test_roundtrip_tabular_tjson_to_ttoon_to_tjson() {
    let original_tjson = r#"[{"id": 1, "val": "x"}, {"id": 2, "val": "y"}]"#;

    let ttoon = crate::tjson_to_ttoon(original_tjson, None).unwrap();
    let round_tjson = crate::ttoon_to_tjson(&ttoon, ParseMode::Compat, None).unwrap();

    let ir_original = crate::tjson_parser::parse_structure(original_tjson).unwrap();
    let ir_round = crate::tjson_parser::parse_structure(&round_tjson).unwrap();
    assert_eq!(ir_original, ir_round);
}

#[test]
fn test_roundtrip_idempotency_second_pass() {
    // 驗證 serialize → parse → serialize 後輸出穩定
    let input = r#"{"a": 1, "b": [2, 3], "c": true}"#;

    let ttoon1 = crate::tjson_to_ttoon(input, None).unwrap();
    let ttoon2 = crate::tjson_to_ttoon(
        &crate::ttoon_to_tjson(&ttoon1, ParseMode::Compat, None).unwrap(),
        None,
    )
    .unwrap();

    // IR comparison for idempotency
    let ir1 = crate::ttoon_parser::parse_ttoon(&ttoon1, ParseMode::Compat).unwrap();
    let ir2 = crate::ttoon_parser::parse_ttoon(&ttoon2, ParseMode::Compat).unwrap();
    assert_eq!(ir1, ir2, "second pass should produce identical IR");
}

#[test]
fn test_roundtrip_typed_values() {
    // Typed values should survive roundtrip
    let cases = vec![
        "3.14m",                                      // decimal
        "2026-03-01",                                 // date
        "12:30:00",                                   // time
        "uuid(550e8400-e29b-41d4-a716-446655440000)", // uuid
        "2026-03-01T12:00:00Z",                       // datetime
    ];

    for input in cases {
        let tjson = crate::ttoon_to_tjson(input, ParseMode::Compat, None).unwrap();
        let ttoon = crate::tjson_to_ttoon(&tjson, None).unwrap();
        let ir_original = crate::ttoon_parser::parse_ttoon(input, ParseMode::Compat).unwrap();
        let ir_round = crate::ttoon_parser::parse_ttoon(ttoon.trim(), ParseMode::Compat).unwrap();
        assert_eq!(ir_original, ir_round, "roundtrip failed for: {}", input);
    }
}

#[test]
fn test_transcode_with_encode_options() {
    let tjson = r#"[{"a": 1, "b": 2}, {"a": 3, "b": 4}]"#;
    let opts = TtoonOptions {
        delimiter: Delimiter::Pipe,
        ..Default::default()
    };
    let ttoon = crate::tjson_to_ttoon(tjson, Some(&opts)).unwrap();
    assert!(
        ttoon.contains("[2|]"),
        "expected pipe delimiter, got: {:?}",
        ttoon
    );
}

#[test]
fn test_transcode_error_kind() {
    let err = crate::tjson_to_ttoon("not valid json {{{", None).unwrap_err();
    assert_eq!(err.kind, ErrorKind::TranscodeError);
    assert!(err
        .message
        .starts_with("tjson_to_ttoon: parse phase failed:"));
    let transcode = err
        .transcode
        .as_ref()
        .expect("expected structured transcode error");
    assert_eq!(transcode.operation, TranscodeOperation::TjsonToTtoon);
    assert_eq!(transcode.phase, TranscodePhase::Parse);
    assert_eq!(transcode.source_kind, ErrorKind::ParseError);
    assert!(!transcode.source.message.is_empty());
}
