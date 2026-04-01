use std::io::Cursor;
use std::sync::Arc;

use arrow_array::{FixedSizeBinaryArray, Int64Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema as ArrowSchema};
use indexmap::IndexMap;

use super::*;
use crate::ir::Node;
use crate::{ErrorKind, FieldType, Result, ScalarType, StreamSchema, TjsonOptions, TtoonOptions};

fn ttoon_schema() -> StreamSchema {
    StreamSchema::try_new([
        ("name", FieldType::new(ScalarType::String)),
        ("age", FieldType::new(ScalarType::Int)),
        ("balance", FieldType::nullable(ScalarType::decimal(10, 2))),
        ("created", FieldType::new(ScalarType::datetime())),
    ])
    .unwrap()
}

fn ttoon_row(name: &str, age: i64, balance: Option<&str>, created: &str) -> IndexMap<String, Node> {
    let mut row = IndexMap::new();
    row.insert("name".to_string(), Node::String(name.to_string()));
    row.insert("age".to_string(), Node::Int(age));
    row.insert(
        "balance".to_string(),
        balance
            .map(|value| Node::Decimal(value.to_string()))
            .unwrap_or(Node::Null),
    );
    row.insert("created".to_string(), Node::DateTime(created.to_string()));
    row
}

fn simple_schema() -> StreamSchema {
    StreamSchema::try_new([
        ("name", FieldType::new(ScalarType::String)),
        ("score", FieldType::new(ScalarType::Int)),
    ])
    .unwrap()
}

fn tjson_schema() -> StreamSchema {
    StreamSchema::try_new([
        ("name", FieldType::new(ScalarType::String)),
        ("age", FieldType::nullable(ScalarType::Int)),
        ("joined", FieldType::nullable(ScalarType::datetime())),
    ])
    .unwrap()
}

#[test]
fn test_stream_writer_reader_roundtrip() {
    let schema = ttoon_schema();
    let mut output = Vec::new();
    let mut writer = StreamWriter::new(&mut output, schema.clone(), TtoonOptions::default());

    let r1 = ttoon_row("Alice", 30, Some("123.45m"), "2026-03-20T12:00:00Z");
    let r2 = ttoon_row("Bob", 31, None, "2026-03-21T12:00:00Z");
    writer.write(&r1).unwrap();
    writer.write(&r2).unwrap();
    let result = writer.close().unwrap();

    assert_eq!(result.rows_emitted, 2);
    let reader = StreamReader::new(Cursor::new(&output), schema);
    let rows = reader.collect::<Result<Vec<_>>>().unwrap();
    assert_eq!(rows, vec![r1, r2]);
}

#[test]
fn test_stream_reader_rejects_header_mismatch() {
    let input = "[*]{age,name,balance,created}:\n30, \"Alice\", 123.45m, 2026-03-20T12:00:00Z\n";
    let mut reader = StreamReader::new(Cursor::new(input.as_bytes()), ttoon_schema());
    let err = reader.next().unwrap().unwrap_err();

    assert_eq!(err.kind, ErrorKind::ParseError);
    assert!(err.message.contains("header field"));
}

#[test]
fn test_arrow_stream_reader_reads_record_batches() {
    let input = "[*]{name,score}:\n\"Alice\", 95\n\"Bob\", 87\n";
    let reader = ArrowStreamReader::new(Cursor::new(input.as_bytes()), simple_schema(), 1).unwrap();
    let batches = reader.collect::<Result<Vec<_>>>().unwrap();

    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0].num_rows(), 1);
    assert_eq!(batches[1].num_rows(), 1);
}

#[test]
fn test_arrow_stream_writer_roundtrip() {
    let schema = simple_schema();
    let arrow_schema = Arc::new(schema.to_arrow_schema().unwrap());
    let batch = RecordBatch::try_new(
        arrow_schema,
        vec![
            Arc::new(StringArray::from(vec!["Alice", "Bob"])),
            Arc::new(Int64Array::from(vec![95_i64, 87])),
        ],
    )
    .unwrap();

    let mut output = Vec::new();
    let mut writer =
        ArrowStreamWriter::new(&mut output, schema.clone(), TtoonOptions::default()).unwrap();
    writer.write_batch(&batch).unwrap();
    let result = writer.close().unwrap();

    assert_eq!(result.rows_emitted, 2);
    let reader = ArrowStreamReader::new(Cursor::new(&output), schema, 4).unwrap();
    let batches = reader.collect::<Result<Vec<_>>>().unwrap();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].num_rows(), 2);
}

#[test]
fn test_arrow_stream_writer_accepts_canonical_uuid_extension_field() {
    let schema = StreamSchema::try_new([("id", FieldType::new(ScalarType::Uuid))]).unwrap();
    let arrow_schema = Arc::new(ArrowSchema::new(vec![Field::new(
        "id",
        DataType::FixedSizeBinary(16),
        false,
    )
    .with_metadata(
        [
            ("ARROW:extension:name".to_owned(), "arrow.uuid".to_owned()),
            ("ARROW:extension:metadata".to_owned(), "".to_owned()),
        ]
        .into_iter()
        .collect(),
    )]));
    let batch = RecordBatch::try_new(
        arrow_schema,
        vec![Arc::new(
            FixedSizeBinaryArray::try_from_iter(
                [vec![0_u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]].into_iter(),
            )
            .unwrap(),
        )],
    )
    .unwrap();

    let mut output = Vec::new();
    let mut writer = ArrowStreamWriter::new(&mut output, schema, TtoonOptions::default()).unwrap();
    writer.write_batch(&batch).unwrap();
    let result = writer.close().unwrap();

    assert_eq!(result.rows_emitted, 1);
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "[*]{id}:\nuuid(00000000-0000-0000-0000-000000000001)\n"
    );
}

#[test]
fn test_tjson_stream_reader_compat_discards_unknown_key_and_materializes_missing_null() {
    let input = r#"[{"name": "Alice", "extra": {"ignored": true}}]"#;
    let mut reader = TjsonStreamReader::new(Cursor::new(input.as_bytes()), tjson_schema());
    let row = reader.next().unwrap().unwrap();

    assert_eq!(row.get("name"), Some(&Node::String("Alice".to_string())));
    assert_eq!(row.get("age"), Some(&Node::Null));
    assert_eq!(row.get("joined"), Some(&Node::Null));
    assert!(reader.next().is_none());
}

#[test]
fn test_tjson_stream_reader_strict_rejects_unknown_key() {
    let input = r#"[{"name": "Alice", "extra": 1}]"#;
    let mut reader = TjsonStreamReader::with_mode(
        Cursor::new(input.as_bytes()),
        tjson_schema(),
        crate::ParseMode::Strict,
    );
    let err = reader.next().unwrap().unwrap_err();

    assert_eq!(err.kind, ErrorKind::ParseError);
    assert!(err.message.contains("unexpected field 'extra'"));
}

#[test]
fn test_tjson_stream_reader_duplicate_key_is_last_write_wins() {
    let input = r#"[{"name": "Alice", "age": 1, "age": 2}]"#;
    let mut reader = TjsonStreamReader::new(Cursor::new(input.as_bytes()), tjson_schema());
    let row = reader.next().unwrap().unwrap();

    assert_eq!(row.get("age"), Some(&Node::Int(2)));
}

#[test]
fn test_tjson_stream_reader_rejects_nested_known_value() {
    let input = r#"[{"name": {"first": "Alice"}}]"#;
    let mut reader = TjsonStreamReader::new(Cursor::new(input.as_bytes()), tjson_schema());
    let err = reader.next().unwrap().unwrap_err();

    assert_eq!(err.kind, ErrorKind::ParseError);
    assert!(err.message.contains("contains non-scalar value"));
}

#[test]
fn test_tjson_arrow_stream_reader_batches_rows() {
    let input = r#"[{"name": "Alice", "age": 1}, {"name": "Bob", "age": null}]"#;
    let reader =
        TjsonArrowStreamReader::new(Cursor::new(input.as_bytes()), tjson_schema(), 1).unwrap();
    let batches = reader.collect::<Result<Vec<_>>>().unwrap();

    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0].num_rows(), 1);
    assert_eq!(batches[1].num_rows(), 1);
}

#[test]
fn test_tjson_stream_writer_empty_close_outputs_empty_array() {
    let mut output = Vec::new();
    let mut writer = TjsonStreamWriter::new(&mut output, tjson_schema(), TjsonOptions::default());

    let result = writer.close().unwrap();
    assert_eq!(result.rows_emitted, 0);
    assert_eq!(String::from_utf8(output).unwrap(), "[]");
}

#[test]
fn test_tjson_stream_writer_schema_first_order() {
    let schema = tjson_schema();
    let mut row = IndexMap::new();
    row.insert("joined".to_string(), Node::Null);
    row.insert("age".to_string(), Node::Int(3));
    row.insert("name".to_string(), Node::String("Alice".to_string()));

    let mut output = Vec::new();
    let mut writer = TjsonStreamWriter::new(&mut output, schema, TjsonOptions::default());
    writer.write(&row).unwrap();
    writer.close().unwrap();

    assert_eq!(
        String::from_utf8(output).unwrap(),
        r#"[{"name": "Alice", "age": 3, "joined": null}]"#
    );
}

#[test]
fn test_tjson_reader_writer_roundtrip_materializes_missing_field_to_null() {
    let input = r#"[{"name": "Alice"}]"#;
    let schema = tjson_schema();
    let mut reader = TjsonStreamReader::new(Cursor::new(input.as_bytes()), schema.clone());
    let row = reader.next().unwrap().unwrap();

    let mut output = Vec::new();
    let mut writer = TjsonStreamWriter::new(&mut output, schema, TjsonOptions::default());
    writer.write(&row).unwrap();
    writer.close().unwrap();

    assert_eq!(
        String::from_utf8(output).unwrap(),
        r#"[{"name": "Alice", "age": null, "joined": null}]"#
    );
}

#[test]
fn test_tjson_stream_writer_failed_state_blocks_close() {
    let mut output = Vec::new();
    let mut writer = TjsonStreamWriter::new(&mut output, tjson_schema(), TjsonOptions::default());

    let mut invalid = IndexMap::new();
    invalid.insert("name".to_string(), Node::String("Alice".to_string()));
    invalid.insert("age".to_string(), Node::Int(1));

    let err = writer.write(&invalid).unwrap_err();
    assert_eq!(err.kind, ErrorKind::SerializeError);

    let close_err = writer.close().unwrap_err();
    assert_eq!(close_err.kind, ErrorKind::SerializeError);
    assert!(close_err.message.contains("failed state"));
    assert!(output.is_empty());
}

#[test]
fn test_tjson_arrow_stream_writer_writes_batches() {
    let schema = StreamSchema::try_new([
        ("name", FieldType::new(ScalarType::String)),
        ("age", FieldType::nullable(ScalarType::Int)),
    ])
    .unwrap();
    let arrow_schema = Arc::new(ArrowSchema::new(vec![
        Field::new("name", DataType::Utf8, false),
        Field::new("age", DataType::Int64, true),
    ]));
    let batch = RecordBatch::try_new(
        arrow_schema,
        vec![
            Arc::new(StringArray::from(vec!["Alice"])),
            Arc::new(Int64Array::from(vec![Some(1_i64)])),
        ],
    )
    .unwrap();

    let mut output = Vec::new();
    let mut writer =
        TjsonArrowStreamWriter::new(&mut output, schema, TjsonOptions::default()).unwrap();
    writer.write_batch(&batch).unwrap();
    writer.close().unwrap();

    assert_eq!(
        String::from_utf8(output).unwrap(),
        r#"[{"name": "Alice", "age": 1}]"#
    );
}

#[test]
fn test_tjson_arrow_stream_writer_rejects_non_nullable_null_and_enters_failed_state() {
    let batch_schema = Arc::new(ArrowSchema::new(vec![
        Field::new("name", DataType::Utf8, true),
        Field::new("score", DataType::Int64, false),
    ]));
    let batch = RecordBatch::try_new(
        batch_schema,
        vec![
            Arc::new(StringArray::from(vec![Some("Alice"), None])),
            Arc::new(Int64Array::from(vec![95_i64, 87])),
        ],
    )
    .unwrap();

    let mut output = Vec::new();
    let mut writer =
        TjsonArrowStreamWriter::new(&mut output, simple_schema(), TjsonOptions::default()).unwrap();
    let err = writer.write_batch(&batch).unwrap_err();

    assert_eq!(err.kind, ErrorKind::SerializeError);
    let close_err = writer.close().unwrap_err();
    assert!(close_err.message.contains("failed state"));
}
