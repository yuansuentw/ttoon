//! WASM bridge for @ttoon/shared.
//!
//! Exposes ttoon-core batch and streaming APIs to JavaScript via wasm-bindgen.
//! Data transfer protocol:
//! - Text ↔ WASM: UTF-8 string
//! - IR ↔ WASM: JSON wire format (see wire.rs)
//! - Arrow ↔ WASM: IPC stream bytes
//! - T-TOON Options: JSON string `{"delimiter":"," | "\t" | "|", "indent_size":2, "binary_format":"hex" | "b64"}`
//! - T-JSON Options: JSON string `{"binary_format":"hex" | "b64"}`

mod stream_handles;
mod wire;

use std::io::{BufReader, Cursor};

use arrow_ipc::reader::StreamReader as IpcStreamReader;
use arrow_ipc::writer::StreamWriter as IpcStreamWriter;
use wasm_bindgen::prelude::*;

use ttoon_core::ir::Node;
use ttoon_core::schema::{FieldType, ScalarType, StreamSchema};
use ttoon_core::streaming::{
    ArrowStreamReader, ArrowStreamWriter, StreamReader, StreamWriter, TjsonArrowStreamReader,
    TjsonArrowStreamWriter, TjsonStreamReader, TjsonStreamWriter,
};
use ttoon_core::typed_parse::ParseMode;
use ttoon_core::{Delimiter, TjsonOptions, TtoonOptions};

use wire::{json_to_node, node_to_json};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn ttoon_err(e: ttoon_core::Error) -> JsError {
    JsError::new(&e.message)
}

fn parse_mode(mode: &str) -> ParseMode {
    match mode {
        "strict" => ParseMode::Strict,
        _ => ParseMode::Compat,
    }
}

fn parse_ttoon_opts(opts_json: &str) -> TtoonOptions {
    if opts_json.is_empty() {
        return TtoonOptions::default();
    }
    let v: serde_json::Value = match serde_json::from_str(opts_json) {
        Ok(v) => v,
        Err(_) => return TtoonOptions::default(),
    };
    let delimiter = match v.get("delimiter").and_then(|d| d.as_str()) {
        Some("\t") => Delimiter::Tab,
        Some("|") => Delimiter::Pipe,
        _ => Delimiter::Comma,
    };
    let indent_size = v.get("indent_size").and_then(|i| i.as_u64()).unwrap_or(2) as u8;
    let binary_format = match v.get("binary_format").and_then(|b| b.as_str()) {
        Some("b64") => ttoon_core::BinaryFormat::B64,
        _ => ttoon_core::BinaryFormat::Hex,
    };
    TtoonOptions {
        delimiter,
        indent_size,
        binary_format,
    }
}

fn parse_tjson_opts(opts_json: &str) -> TjsonOptions {
    if opts_json.is_empty() {
        return TjsonOptions::default();
    }
    let v: serde_json::Value = match serde_json::from_str(opts_json) {
        Ok(v) => v,
        Err(_) => return TjsonOptions::default(),
    };
    let binary_format = match v.get("binary_format").and_then(|b| b.as_str()) {
        Some("b64") => ttoon_core::BinaryFormat::B64,
        _ => ttoon_core::BinaryFormat::Hex,
    };
    TjsonOptions { binary_format }
}

// ─── Batch: Parse ────────────────────────────────────────────────────────────

/// Parse T-TOON or T-JSON text → JSON IR string.
#[wasm_bindgen]
pub fn parse(text: &str, mode: &str) -> Result<String, JsError> {
    let node = ttoon_core::from_ttoon_with_mode(text, parse_mode(mode)).map_err(ttoon_err)?;
    node_to_json(&node).map_err(|e| JsError::new(&e))
}

// ─── Batch: Stringify ────────────────────────────────────────────────────────

/// Serialize JSON IR string → T-TOON text.
#[wasm_bindgen]
pub fn stringify_ttoon(ir_json: &str, opts_json: &str) -> Result<String, JsError> {
    let node = json_to_node(ir_json).map_err(|e| JsError::new(&e))?;
    let opts = parse_ttoon_opts(opts_json);
    ttoon_core::to_ttoon(&node, Some(&opts)).map_err(ttoon_err)
}

/// Serialize JSON IR string → T-JSON text.
#[wasm_bindgen]
pub fn stringify_tjson(ir_json: &str, opts_json: &str) -> Result<String, JsError> {
    let node = json_to_node(ir_json).map_err(|e| JsError::new(&e))?;
    let opts = parse_tjson_opts(opts_json);
    ttoon_core::to_tjson(&node, Some(&opts)).map_err(ttoon_err)
}

// ─── Batch: Arrow ────────────────────────────────────────────────────────────

/// Parse text → Arrow IPC stream bytes.
#[wasm_bindgen]
pub fn read_arrow(text: &str) -> Result<Vec<u8>, JsError> {
    let table = ttoon_core::read_arrow(text).map_err(ttoon_err)?;

    let mut ipc_buf: Vec<u8> = Vec::new();
    let mut ipc_writer = IpcStreamWriter::try_new(&mut ipc_buf, table.schema.as_ref())
        .map_err(|e| JsError::new(&e.to_string()))?;
    for batch in &table.batches {
        ipc_writer
            .write(batch)
            .map_err(|e| JsError::new(&e.to_string()))?;
    }
    ipc_writer
        .finish()
        .map_err(|e| JsError::new(&e.to_string()))?;

    Ok(ipc_buf)
}

/// Arrow IPC stream bytes → T-TOON tabular text.
#[wasm_bindgen]
pub fn stringify_arrow_ttoon(ipc_bytes: &[u8], opts_json: &str) -> Result<String, JsError> {
    let table = ipc_to_arrow_table(ipc_bytes)?;
    let opts = parse_ttoon_opts(opts_json);
    ttoon_core::arrow_to_ttoon(&table, Some(&opts)).map_err(ttoon_err)
}

/// Arrow IPC stream bytes → T-JSON text.
#[wasm_bindgen]
pub fn stringify_arrow_tjson(ipc_bytes: &[u8], opts_json: &str) -> Result<String, JsError> {
    let table = ipc_to_arrow_table(ipc_bytes)?;
    let opts = parse_tjson_opts(opts_json);
    ttoon_core::arrow_to_tjson(&table, Some(&opts)).map_err(ttoon_err)
}

fn ipc_to_arrow_table(ipc_bytes: &[u8]) -> Result<ttoon_core::ir::ArrowTable, JsError> {
    let cursor = Cursor::new(ipc_bytes);
    let reader =
        IpcStreamReader::try_new(cursor, None).map_err(|e| JsError::new(&e.to_string()))?;

    let schema = reader.schema();
    let batches: Vec<_> = reader
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| JsError::new(&e.to_string()))?;

    if batches.is_empty() {
        return Err(JsError::new("empty Arrow IPC stream"));
    }

    Ok(ttoon_core::ir::ArrowTable { schema, batches })
}

// ─── Direct Transcode ────────────────────────────────────────────────────────

/// T-JSON text → T-TOON text (no JS middle layer).
#[wasm_bindgen]
pub fn tjson_to_ttoon(text: &str, opts_json: &str) -> Result<String, JsError> {
    let opts = parse_ttoon_opts(opts_json);
    ttoon_core::tjson_to_ttoon(text, Some(&opts)).map_err(ttoon_err)
}

/// T-TOON text → T-JSON text (no JS middle layer).
#[wasm_bindgen]
pub fn ttoon_to_tjson(text: &str, mode: &str, opts_json: &str) -> Result<String, JsError> {
    let opts = parse_tjson_opts(opts_json);
    ttoon_core::ttoon_to_tjson(text, parse_mode(mode), Some(&opts)).map_err(ttoon_err)
}

// ─── Format Detection ────────────────────────────────────────────────────────

/// Detect input format: "ttoon", "tjson", or "typed_unit".
#[wasm_bindgen]
pub fn detect_format(text: &str) -> String {
    let fmt = ttoon_core::detect_format(text);
    format!("{:?}", fmt).to_lowercase()
}

// ─── Streaming: Arrow ────────────────────────────────────────────────────────

/// Build a StreamSchema from a JSON field definition.
/// Input: `[{"name":"field","type":"string","nullable":false}, ...]`
fn schema_from_json(schema_json: &str) -> Result<StreamSchema, JsError> {
    let fields: Vec<serde_json::Value> =
        serde_json::from_str(schema_json).map_err(|e| JsError::new(&e.to_string()))?;

    let mut schema_fields: Vec<(String, FieldType)> = Vec::new();
    for field in fields {
        let name = field
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| JsError::new("field missing 'name'"))?
            .to_string();
        let type_str = field
            .get("type")
            .and_then(|t| t.as_str())
            .ok_or_else(|| JsError::new("field missing 'type'"))?;
        let nullable = field
            .get("nullable")
            .and_then(|n| n.as_bool())
            .unwrap_or(false);

        let scalar = parse_scalar_type(type_str, &field)?;
        let ft = if nullable {
            FieldType::nullable(scalar)
        } else {
            FieldType::new(scalar)
        };
        schema_fields.push((name, ft));
    }

    Ok(StreamSchema::new(schema_fields))
}

fn parse_scalar_type(type_str: &str, field: &serde_json::Value) -> Result<ScalarType, JsError> {
    match type_str {
        "string" => Ok(ScalarType::String),
        "int" => Ok(ScalarType::Int),
        "float" => Ok(ScalarType::Float),
        "bool" => Ok(ScalarType::Bool),
        "date" => Ok(ScalarType::Date),
        "time" => Ok(ScalarType::Time),
        "uuid" => Ok(ScalarType::Uuid),
        "binary" => Ok(ScalarType::Binary),
        "decimal" => {
            let precision = field
                .get("precision")
                .and_then(|p| p.as_u64())
                .unwrap_or(38) as u8;
            let scale = field.get("scale").and_then(|s| s.as_i64()).unwrap_or(0) as i8;
            Ok(ScalarType::Decimal { precision, scale })
        }
        "datetime" => {
            let has_tz = field
                .get("has_tz")
                .and_then(|t| t.as_bool())
                .unwrap_or(false);
            Ok(ScalarType::DateTime { has_tz })
        }
        _ => Err(JsError::new(&format!("unknown scalar type '{}'", type_str))),
    }
}

// ─── Streaming: Arrow parse (text → IPC) ────────────────────────────────────

/// Streaming parse: text + schema → Arrow IPC stream bytes.
#[wasm_bindgen]
pub fn stream_parse_arrow(
    text: &str,
    schema_json: &str,
    batch_size: usize,
    mode: &str,
) -> Result<Vec<u8>, JsError> {
    let schema = schema_from_json(schema_json)?;
    let reader = BufReader::new(Cursor::new(text));
    let arrow_reader = ArrowStreamReader::with_mode(reader, schema, batch_size, parse_mode(mode))
        .map_err(ttoon_err)?;

    let mut ipc_buf: Vec<u8> = Vec::new();
    let mut ipc_writer: Option<IpcStreamWriter<&mut Vec<u8>>> = None;

    for batch_result in arrow_reader {
        let batch = batch_result.map_err(ttoon_err)?;

        if ipc_writer.is_none() {
            let writer = IpcStreamWriter::try_new(&mut ipc_buf, batch.schema().as_ref())
                .map_err(|e| JsError::new(&e.to_string()))?;
            ipc_writer = Some(writer);
        }

        if let Some(ref mut w) = ipc_writer {
            w.write(&batch).map_err(|e| JsError::new(&e.to_string()))?;
        }
    }

    if let Some(mut w) = ipc_writer {
        w.finish().map_err(|e| JsError::new(&e.to_string()))?;
    }

    Ok(ipc_buf)
}

// ─── Streaming: Arrow serialize (IPC → text) ────────────────────────────────

/// Streaming serialize: Arrow IPC bytes + schema → T-TOON text.
#[wasm_bindgen]
pub fn stream_stringify_arrow(
    ipc_bytes: &[u8],
    schema_json: &str,
    opts_json: &str,
) -> Result<String, JsError> {
    let schema = schema_from_json(schema_json)?;
    let opts = parse_ttoon_opts(opts_json);

    let mut text_buf: Vec<u8> = Vec::new();
    let mut writer = ArrowStreamWriter::new(&mut text_buf, schema, opts).map_err(ttoon_err)?;

    let cursor = Cursor::new(ipc_bytes);
    let ipc_reader =
        IpcStreamReader::try_new(cursor, None).map_err(|e| JsError::new(&e.to_string()))?;

    for batch_result in ipc_reader {
        let batch = batch_result.map_err(|e| JsError::new(&e.to_string()))?;
        writer.write_batch(&batch).map_err(ttoon_err)?;
    }

    writer.close().map_err(ttoon_err)?;
    String::from_utf8(text_buf).map_err(|e| JsError::new(&e.to_string()))
}

// ─── Streaming: Object parse (text → JSON rows) ─────────────────────────────

/// Streaming parse: text + schema → JSON IR rows (array of objects).
#[wasm_bindgen]
pub fn stream_parse_object(text: &str, schema_json: &str) -> Result<String, JsError> {
    let schema = schema_from_json(schema_json)?;
    let reader = BufReader::new(Cursor::new(text));
    let stream_reader = StreamReader::with_mode(reader, schema, ParseMode::Compat);

    let mut rows: Vec<wire::WireNode> = Vec::new();
    for row_result in stream_reader {
        let row = row_result.map_err(ttoon_err)?;
        let node = Node::Object(row);
        rows.push(wire::WireNode::from(&node));
    }

    let wire_list = wire::WireNode::List { v: rows };
    serde_json::to_string(&wire_list).map_err(|e| JsError::new(&e.to_string()))
}

/// Streaming parse: text + schema → JSON IR rows (array of objects).
#[wasm_bindgen]
pub fn stream_parse_object_with_mode(
    text: &str,
    schema_json: &str,
    mode: &str,
) -> Result<String, JsError> {
    let schema = schema_from_json(schema_json)?;
    let reader = BufReader::new(Cursor::new(text));
    let stream_reader = StreamReader::with_mode(reader, schema, parse_mode(mode));

    let mut rows: Vec<wire::WireNode> = Vec::new();
    for row_result in stream_reader {
        let row = row_result.map_err(ttoon_err)?;
        let node = Node::Object(row);
        rows.push(wire::WireNode::from(&node));
    }

    let wire_list = wire::WireNode::List { v: rows };
    serde_json::to_string(&wire_list).map_err(|e| JsError::new(&e.to_string()))
}

// ─── Streaming: Object serialize (JSON rows → text) ──────────────────────────

/// Streaming serialize: JSON IR (list of objects) + schema → T-TOON text.
#[wasm_bindgen]
pub fn stream_stringify_object(
    ir_json: &str,
    schema_json: &str,
    opts_json: &str,
) -> Result<String, JsError> {
    let node = json_to_node(ir_json).map_err(|e| JsError::new(&e))?;
    let schema = schema_from_json(schema_json)?;
    let opts = parse_ttoon_opts(opts_json);

    let rows = match node {
        Node::List(items) => items,
        _ => {
            return Err(JsError::new(
                "expected list of objects for streaming serialize",
            ))
        }
    };

    let mut text_buf: Vec<u8> = Vec::new();
    let mut writer = StreamWriter::new(&mut text_buf, schema, opts);

    for row_node in &rows {
        match row_node {
            Node::Object(map) => {
                writer.write(map).map_err(ttoon_err)?;
            }
            _ => return Err(JsError::new("expected object in row list")),
        }
    }

    writer.close().map_err(ttoon_err)?;
    String::from_utf8(text_buf).map_err(|e| JsError::new(&e.to_string()))
}

// ─── Streaming: T-JSON parse (text → IPC / JSON rows) ───────────────────────

/// Streaming parse: T-JSON text + schema → Arrow IPC stream bytes.
#[wasm_bindgen]
pub fn stream_parse_arrow_tjson(
    text: &str,
    schema_json: &str,
    batch_size: usize,
    mode: &str,
) -> Result<Vec<u8>, JsError> {
    let schema = schema_from_json(schema_json)?;
    let reader = BufReader::new(Cursor::new(text));
    let arrow_reader =
        TjsonArrowStreamReader::with_mode(reader, schema, batch_size, parse_mode(mode))
            .map_err(ttoon_err)?;

    let mut ipc_buf: Vec<u8> = Vec::new();
    let mut ipc_writer: Option<IpcStreamWriter<&mut Vec<u8>>> = None;

    for batch_result in arrow_reader {
        let batch = batch_result.map_err(ttoon_err)?;

        if ipc_writer.is_none() {
            let writer = IpcStreamWriter::try_new(&mut ipc_buf, batch.schema().as_ref())
                .map_err(|e| JsError::new(&e.to_string()))?;
            ipc_writer = Some(writer);
        }

        if let Some(ref mut w) = ipc_writer {
            w.write(&batch).map_err(|e| JsError::new(&e.to_string()))?;
        }
    }

    if let Some(mut w) = ipc_writer {
        w.finish().map_err(|e| JsError::new(&e.to_string()))?;
    }

    Ok(ipc_buf)
}

/// Streaming parse: T-JSON text + schema → JSON IR rows (array of objects).
#[wasm_bindgen]
pub fn stream_parse_object_tjson(
    text: &str,
    schema_json: &str,
    mode: &str,
) -> Result<String, JsError> {
    let schema = schema_from_json(schema_json)?;
    let reader = BufReader::new(Cursor::new(text));
    let stream_reader = TjsonStreamReader::with_mode(reader, schema, parse_mode(mode));

    let mut rows: Vec<wire::WireNode> = Vec::new();
    for row_result in stream_reader {
        let row = row_result.map_err(ttoon_err)?;
        let node = Node::Object(row);
        rows.push(wire::WireNode::from(&node));
    }

    let wire_list = wire::WireNode::List { v: rows };
    serde_json::to_string(&wire_list).map_err(|e| JsError::new(&e.to_string()))
}

// ─── Streaming: T-JSON serialize (IPC / JSON rows → text) ───────────────────

/// Streaming serialize: Arrow IPC bytes + schema → T-JSON text.
#[wasm_bindgen]
pub fn stream_stringify_arrow_tjson(
    ipc_bytes: &[u8],
    schema_json: &str,
    opts_json: &str,
) -> Result<String, JsError> {
    let schema = schema_from_json(schema_json)?;
    let opts = parse_tjson_opts(opts_json);

    let mut text_buf: Vec<u8> = Vec::new();
    let mut writer = TjsonArrowStreamWriter::new(&mut text_buf, schema, opts).map_err(ttoon_err)?;

    let cursor = Cursor::new(ipc_bytes);
    let ipc_reader =
        IpcStreamReader::try_new(cursor, None).map_err(|e| JsError::new(&e.to_string()))?;

    for batch_result in ipc_reader {
        let batch = batch_result.map_err(|e| JsError::new(&e.to_string()))?;
        writer.write_batch(&batch).map_err(ttoon_err)?;
    }

    writer.close().map_err(ttoon_err)?;
    String::from_utf8(text_buf).map_err(|e| JsError::new(&e.to_string()))
}

/// Streaming serialize: JSON IR (list of objects) + schema → T-JSON text.
#[wasm_bindgen]
pub fn stream_stringify_object_tjson(
    ir_json: &str,
    schema_json: &str,
    opts_json: &str,
) -> Result<String, JsError> {
    let node = json_to_node(ir_json).map_err(|e| JsError::new(&e))?;
    let schema = schema_from_json(schema_json)?;
    let opts = parse_tjson_opts(opts_json);

    let rows = match node {
        Node::List(items) => items,
        _ => {
            return Err(JsError::new(
                "expected list of objects for streaming serialize",
            ))
        }
    };

    let mut text_buf: Vec<u8> = Vec::new();
    let mut writer = TjsonStreamWriter::new(&mut text_buf, schema, opts);

    for row_node in &rows {
        match row_node {
            Node::Object(map) => {
                writer.write(map).map_err(ttoon_err)?;
            }
            _ => return Err(JsError::new("expected object in row list")),
        }
    }

    writer.close().map_err(ttoon_err)?;
    String::from_utf8(text_buf).map_err(|e| JsError::new(&e.to_string()))
}
