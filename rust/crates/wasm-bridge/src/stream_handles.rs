use std::cell::RefCell;
use std::io::{Cursor, Write};
use std::rc::Rc;

use arrow_array::RecordBatch;
use arrow_ipc::reader::StreamReader as IpcStreamReader;
use arrow_ipc::writer::StreamWriter as IpcStreamWriter;
use indexmap::IndexMap;
use wasm_bindgen::prelude::*;

use ttoon_core::ir::Node;
use ttoon_core::streaming::{
    ArrowStreamReader, ArrowStreamWriter, StreamReader, StreamWriter, TjsonArrowStreamReader,
    TjsonArrowStreamWriter, TjsonStreamReader, TjsonStreamWriter,
};
use ttoon_core::{Delimiter, ParseMode, StreamSchema};

use crate::wire::{json_to_node, node_to_json};
use crate::{parse_mode, parse_tjson_opts, parse_ttoon_opts, schema_from_json, ttoon_err};

const EMPTY_WIRE_LIST_JSON: &str = r#"{"k":"list","v":[]}"#;

#[derive(Clone, Default)]
struct SharedTextSink {
    inner: Rc<RefCell<Vec<u8>>>,
}

impl SharedTextSink {
    fn take_text_since(&self, offset: &mut usize) -> Result<String, JsError> {
        let bytes = self.inner.borrow();
        let chunk = std::str::from_utf8(&bytes[*offset..])
            .map_err(|err| JsError::new(&format!("invalid UTF-8 output: {}", err)))?
            .to_string();
        *offset = bytes.len();
        Ok(chunk)
    }
}

impl Write for SharedTextSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn row_json_to_map(row_json: &str) -> Result<IndexMap<String, Node>, JsError> {
    match json_to_node(row_json).map_err(|err| JsError::new(&err))? {
        Node::Object(map) => Ok(map),
        _ => Err(JsError::new("expected object row")),
    }
}

fn rows_to_wire_json(rows: Vec<IndexMap<String, Node>>) -> Result<String, JsError> {
    if rows.is_empty() {
        return Ok(EMPTY_WIRE_LIST_JSON.to_string());
    }
    let node = Node::List(rows.into_iter().map(Node::Object).collect());
    node_to_json(&node).map_err(|err| JsError::new(&err))
}

fn batches_to_ipc_bytes(batches: Vec<RecordBatch>) -> Result<Vec<u8>, JsError> {
    if batches.is_empty() {
        return Ok(Vec::new());
    }

    let mut ipc_buf = Vec::new();
    let mut writer = IpcStreamWriter::try_new(&mut ipc_buf, batches[0].schema().as_ref())
        .map_err(|err| JsError::new(&err.to_string()))?;
    for batch in batches {
        writer
            .write(&batch)
            .map_err(|err| JsError::new(&err.to_string()))?;
    }
    writer
        .finish()
        .map_err(|err| JsError::new(&err.to_string()))?;
    Ok(ipc_buf)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BridgeTabularRowCount {
    Exact(usize),
    Streaming,
}

#[derive(Debug, Clone)]
struct BridgeTabularHeader {
    row_count: BridgeTabularRowCount,
    delimiter: Delimiter,
    fields: Vec<String>,
}

fn split_bracket_content(content: &str) -> (&str, Delimiter) {
    if content.ends_with('\t') {
        (&content[..content.len() - 1], Delimiter::Tab)
    } else if content.ends_with("\\|") {
        (&content[..content.len() - 2], Delimiter::Pipe)
    } else if content.ends_with('|') {
        (&content[..content.len() - 1], Delimiter::Pipe)
    } else if content.ends_with(',') {
        (&content[..content.len() - 1], Delimiter::Comma)
    } else {
        (content, Delimiter::Comma)
    }
}

fn parse_row_count(raw: &str) -> Result<usize, JsError> {
    if raw.is_empty() {
        return Ok(0);
    }
    raw.parse::<usize>()
        .map_err(|_| JsError::new("invalid row count in header"))
}

fn split_by_delimiter<'a>(text: &'a str, delimiter: Delimiter) -> Vec<&'a str> {
    match delimiter {
        Delimiter::Comma => text.split(',').collect(),
        Delimiter::Tab => text.split('\t').collect(),
        Delimiter::Pipe => text.split('|').collect(),
    }
}

fn parse_bridge_tabular_header(line: &str) -> Result<BridgeTabularHeader, JsError> {
    if !line.starts_with('[') {
        return Err(JsError::new("tabular header must start with '['"));
    }
    let after_bracket = &line[1..];
    let close = after_bracket
        .find(']')
        .ok_or_else(|| JsError::new("missing ']' in tabular header"))?;
    let bracket_content = &after_bracket[..close];
    let after_close = &after_bracket[close + 1..];

    let (count_raw, delimiter) = split_bracket_content(bracket_content);
    let row_count = if count_raw == "*" {
        BridgeTabularRowCount::Streaming
    } else {
        BridgeTabularRowCount::Exact(parse_row_count(count_raw)?)
    };

    if let Some(brace_rest) = after_close.strip_prefix('{') {
        let brace_end = brace_rest
            .find("}:")
            .ok_or_else(|| JsError::new("missing '}:' in tabular header"))?;
        let fields = split_by_delimiter(&brace_rest[..brace_end], delimiter)
            .into_iter()
            .map(|field| field.trim().to_string())
            .collect();
        return Ok(BridgeTabularHeader {
            row_count,
            delimiter,
            fields,
        });
    }

    if after_close == ":" && !matches!(row_count, BridgeTabularRowCount::Streaming) {
        return Ok(BridgeTabularHeader {
            row_count,
            delimiter,
            fields: Vec::new(),
        });
    }

    Err(JsError::new("invalid tabular header"))
}

fn format_streaming_tabular_header(fields: &[String], delimiter: Delimiter) -> String {
    let delim_sym = match delimiter {
        Delimiter::Comma => "",
        Delimiter::Tab => "\t",
        Delimiter::Pipe => "|",
    };
    let delim_join = match delimiter {
        Delimiter::Comma => ",",
        Delimiter::Tab => "\t",
        Delimiter::Pipe => "|",
    };
    format!("[*{}]{{{}}}:", delim_sym, fields.join(delim_join))
}

struct TtoonFeedState {
    schema: StreamSchema,
    mode: ParseMode,
    header: Option<BridgeTabularHeader>,
    line_buf: String,
    rows_read: usize,
    closed: bool,
}

impl TtoonFeedState {
    fn new(schema: StreamSchema, mode: ParseMode) -> Self {
        Self {
            schema,
            mode,
            header: None,
            line_buf: String::new(),
            rows_read: 0,
            closed: false,
        }
    }

    fn feed_rows(&mut self, chunk: &str, finalize: bool) -> Result<Vec<String>, JsError> {
        if self.closed {
            if chunk.trim().is_empty() {
                return Ok(Vec::new());
            }
            return Err(JsError::new("stream already finished"));
        }
        self.line_buf.push_str(chunk);

        let mut row_lines = Vec::new();
        loop {
            let Some(mut line) = self.take_next_line(finalize) else {
                break;
            };
            if self.header.is_none() {
                let trimmed = line.trim_start_matches('\u{FEFF}').trim().to_string();
                if trimmed.is_empty() {
                    continue;
                }
                let header = parse_bridge_tabular_header(&trimmed)?;
                self.validate_header(&header)?;
                if matches!(header.row_count, BridgeTabularRowCount::Exact(0)) {
                    self.header = Some(header);
                    self.closed = true;
                    if !self.line_buf.trim().is_empty() {
                        return Err(JsError::new(
                            "stream_read: header declares 0 rows but trailing data was found",
                        ));
                    }
                    return Ok(Vec::new());
                }
                self.header = Some(header);
                continue;
            }

            line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }

            let header = self.header.as_ref().expect("header initialized");
            if let BridgeTabularRowCount::Exact(expected) = header.row_count {
                if self.rows_read >= expected {
                    return Err(JsError::new(&format!(
                        "stream_read: expected {} rows, but trailing data was found",
                        expected
                    )));
                }
            }

            self.rows_read += 1;
            row_lines.push(line);
        }

        if finalize {
            self.closed = true;
            let header = self
                .header
                .as_ref()
                .ok_or_else(|| JsError::new("stream_read: missing tabular header"))?;
            match header.row_count {
                BridgeTabularRowCount::Streaming if self.rows_read == 0 => {
                    return Err(JsError::new(
                        "stream_read: [*] header requires at least one data row",
                    ))
                }
                BridgeTabularRowCount::Exact(expected) if self.rows_read < expected => {
                    return Err(JsError::new(&format!(
                        "stream_read: expected {} rows, got {}",
                        expected, self.rows_read
                    )))
                }
                _ => {}
            }
        }

        Ok(row_lines)
    }

    fn header(&self) -> Result<&BridgeTabularHeader, JsError> {
        self.header
            .as_ref()
            .ok_or_else(|| JsError::new("stream_read: missing tabular header"))
    }

    fn take_next_line(&mut self, finalize: bool) -> Option<String> {
        if let Some(pos) = self.line_buf.find('\n') {
            let mut line = self.line_buf.drain(..=pos).collect::<String>();
            if line.ends_with('\n') {
                line.pop();
            }
            if line.ends_with('\r') {
                line.pop();
            }
            return Some(line);
        }

        if finalize && !self.line_buf.is_empty() {
            return Some(std::mem::take(&mut self.line_buf));
        }

        None
    }

    fn validate_header(&self, header: &BridgeTabularHeader) -> Result<(), JsError> {
        if header.fields.len() != self.schema.len() {
            return Err(JsError::new(&format!(
                "stream_read: header has {} fields, schema expects {}",
                header.fields.len(),
                self.schema.len()
            )));
        }

        for (expected, actual) in self.schema.fields().iter().zip(header.fields.iter()) {
            if expected.name() != actual {
                return Err(JsError::new(&format!(
                    "stream_read: header field '{}' does not match schema field '{}'",
                    actual,
                    expected.name()
                )));
            }
        }

        Ok(())
    }

    fn parse_rows(&self, rows: Vec<String>) -> Result<Vec<IndexMap<String, Node>>, JsError> {
        if rows.is_empty() {
            return Ok(Vec::new());
        }

        let header = self.header()?;
        let mut text = String::new();
        text.push_str(&format_streaming_tabular_header(
            &header.fields,
            header.delimiter,
        ));
        text.push('\n');
        for row in &rows {
            text.push_str(row);
            text.push('\n');
        }

        let reader =
            StreamReader::with_mode(Cursor::new(text.as_bytes()), self.schema.clone(), self.mode);
        reader
            .collect::<ttoon_core::Result<Vec<_>>>()
            .map_err(ttoon_err)
    }

    fn parse_arrow_batches(
        &self,
        rows: Vec<String>,
        batch_size: usize,
    ) -> Result<Vec<RecordBatch>, JsError> {
        if rows.is_empty() {
            return Ok(Vec::new());
        }

        let header = self.header()?;
        let mut text = String::new();
        text.push_str(&format_streaming_tabular_header(
            &header.fields,
            header.delimiter,
        ));
        text.push('\n');
        for row in &rows {
            text.push_str(row);
            text.push('\n');
        }

        let reader = ArrowStreamReader::with_mode(
            Cursor::new(text.as_bytes()),
            self.schema.clone(),
            batch_size,
            self.mode,
        )
        .map_err(ttoon_err)?;
        reader
            .collect::<ttoon_core::Result<Vec<_>>>()
            .map_err(ttoon_err)
    }
}

struct TjsonArrayScanner {
    buffer: String,
    pos: usize,
    initialized: bool,
    expect_delimiter_or_end: bool,
    finished: bool,
}

impl TjsonArrayScanner {
    fn new() -> Self {
        Self {
            buffer: String::new(),
            pos: 0,
            initialized: false,
            expect_delimiter_or_end: false,
            finished: false,
        }
    }

    fn feed_objects(&mut self, chunk: &str, finalize: bool) -> Result<Vec<String>, JsError> {
        if self.finished {
            if chunk.trim().is_empty() {
                return Ok(Vec::new());
            }
            return Err(JsError::new(
                "stream_read_tjson: unexpected trailing data after array",
            ));
        }

        self.buffer.push_str(chunk);
        let mut objects = Vec::new();

        loop {
            if !self.initialized {
                let Some(start) = self.skip_ws(self.pos) else {
                    if finalize {
                        return Err(JsError::new("stream_read_tjson: unexpected end of input"));
                    }
                    break;
                };
                if self.byte_at(start) != Some(b'[') {
                    return Err(JsError::new(
                        "stream_read_tjson: top-level value must be array",
                    ));
                }
                self.pos = start + 1;
                self.initialized = true;
            }

            if self.expect_delimiter_or_end {
                let Some(index) = self.skip_ws(self.pos) else {
                    if finalize {
                        return Err(JsError::new("stream_read_tjson: unexpected end of input"));
                    }
                    break;
                };
                match self.byte_at(index) {
                    Some(b',') => {
                        self.pos = index + 1;
                        self.expect_delimiter_or_end = false;
                        continue;
                    }
                    Some(b']') => {
                        self.pos = index + 1;
                        self.finished = true;
                        continue;
                    }
                    Some(_) => {
                        return Err(JsError::new("stream_read_tjson: expected ',' or ']'"));
                    }
                    None => break,
                }
            }

            let Some(index) = self.skip_ws(self.pos) else {
                if finalize {
                    return Err(JsError::new("stream_read_tjson: unexpected end of input"));
                }
                break;
            };

            match self.byte_at(index) {
                Some(b']') => {
                    self.pos = index + 1;
                    self.finished = true;
                }
                Some(b'{') => {
                    let Some(end) = self.find_json_object_end(index) else {
                        if finalize {
                            return Err(JsError::new("stream_read_tjson: unexpected end of input"));
                        }
                        break;
                    };
                    objects.push(self.buffer[index..end].to_string());
                    self.pos = end;
                    self.expect_delimiter_or_end = true;
                }
                Some(_) => {
                    return Err(JsError::new(
                        "stream_read_tjson: array items must be objects",
                    ))
                }
                None => break,
            }
        }

        if self.finished {
            let Some(index) = self.skip_ws(self.pos) else {
                self.compact();
                return Ok(objects);
            };
            if index < self.buffer.len() {
                return Err(JsError::new(
                    "stream_read_tjson: unexpected trailing data after array",
                ));
            }
        } else if finalize {
            return Err(JsError::new("stream_read_tjson: unexpected end of input"));
        }

        self.compact();
        Ok(objects)
    }

    fn skip_ws(&self, mut index: usize) -> Option<usize> {
        let bytes = self.buffer.as_bytes();
        while index < bytes.len() {
            match bytes[index] {
                b' ' | b'\n' | b'\r' | b'\t' => index += 1,
                _ => return Some(index),
            }
        }
        None
    }

    fn byte_at(&self, index: usize) -> Option<u8> {
        self.buffer.as_bytes().get(index).copied()
    }

    fn compact(&mut self) {
        if self.pos > 0 {
            self.buffer.drain(..self.pos);
            self.pos = 0;
        }
    }

    fn find_json_object_end(&self, start: usize) -> Option<usize> {
        let bytes = self.buffer.as_bytes();
        let mut depth: i32 = 0;
        let mut in_string = false;
        let mut escaped = false;
        let mut index = start;

        while index < bytes.len() {
            let byte = bytes[index];
            if in_string {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    in_string = false;
                }
                index += 1;
                continue;
            }

            match byte {
                b'"' => in_string = true,
                b'{' | b'[' => depth += 1,
                b'}' | b']' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(index + 1);
                    }
                }
                _ => {}
            }
            index += 1;
        }

        None
    }

    fn parse_rows(
        &self,
        objects: Vec<String>,
        schema: &StreamSchema,
        mode: ParseMode,
    ) -> Result<Vec<IndexMap<String, Node>>, JsError> {
        if objects.is_empty() {
            return Ok(Vec::new());
        }

        let text = format!("[{}]", objects.join(", "));
        let reader =
            TjsonStreamReader::with_mode(Cursor::new(text.as_bytes()), schema.clone(), mode);
        reader
            .collect::<ttoon_core::Result<Vec<_>>>()
            .map_err(ttoon_err)
    }

    fn parse_arrow_batches(
        &self,
        objects: Vec<String>,
        schema: &StreamSchema,
        batch_size: usize,
        mode: ParseMode,
    ) -> Result<Vec<RecordBatch>, JsError> {
        if objects.is_empty() {
            return Ok(Vec::new());
        }

        let text = format!("[{}]", objects.join(", "));
        let reader = TjsonArrowStreamReader::with_mode(
            Cursor::new(text.as_bytes()),
            schema.clone(),
            batch_size,
            mode,
        )
        .map_err(ttoon_err)?;
        reader
            .collect::<ttoon_core::Result<Vec<_>>>()
            .map_err(ttoon_err)
    }
}

#[wasm_bindgen]
pub struct StreamObjectReaderHandle {
    inner: TtoonFeedState,
}

#[wasm_bindgen]
impl StreamObjectReaderHandle {
    #[wasm_bindgen(constructor)]
    pub fn new(schema_json: &str, mode: &str) -> Result<StreamObjectReaderHandle, JsError> {
        let schema = schema_from_json(schema_json)?;
        Ok(Self {
            inner: TtoonFeedState::new(schema, parse_mode(mode)),
        })
    }

    pub fn feed(&mut self, chunk: &str) -> Result<String, JsError> {
        let rows = self.inner.feed_rows(chunk, false)?;
        rows_to_wire_json(self.inner.parse_rows(rows)?)
    }

    pub fn finish(&mut self) -> Result<String, JsError> {
        let rows = self.inner.feed_rows("", true)?;
        rows_to_wire_json(self.inner.parse_rows(rows)?)
    }
}

#[wasm_bindgen]
pub struct StreamArrowReaderHandle {
    inner: TtoonFeedState,
    batch_size: usize,
    row_buffer: Vec<String>,
}

impl StreamArrowReaderHandle {
    fn flush_full_batches(&mut self) -> Result<Vec<RecordBatch>, JsError> {
        let mut all_batches = Vec::new();
        while self.row_buffer.len() >= self.batch_size {
            let chunk: Vec<String> = self.row_buffer.drain(..self.batch_size).collect();
            all_batches.extend(self.inner.parse_arrow_batches(chunk, self.batch_size)?);
        }
        Ok(all_batches)
    }
}

#[wasm_bindgen]
impl StreamArrowReaderHandle {
    #[wasm_bindgen(constructor)]
    pub fn new(
        schema_json: &str,
        batch_size: usize,
        mode: &str,
    ) -> Result<StreamArrowReaderHandle, JsError> {
        let schema = schema_from_json(schema_json)?;
        Ok(Self {
            inner: TtoonFeedState::new(schema, parse_mode(mode)),
            batch_size,
            row_buffer: Vec::new(),
        })
    }

    pub fn feed(&mut self, chunk: &str) -> Result<Vec<u8>, JsError> {
        let rows = self.inner.feed_rows(chunk, false)?;
        self.row_buffer.extend(rows);
        batches_to_ipc_bytes(self.flush_full_batches()?)
    }

    pub fn finish(&mut self) -> Result<Vec<u8>, JsError> {
        let rows = self.inner.feed_rows("", true)?;
        self.row_buffer.extend(rows);
        let mut batches = self.flush_full_batches()?;
        // Flush remaining rows (< batch_size) as final partial batch
        if !self.row_buffer.is_empty() {
            let remaining: Vec<String> = self.row_buffer.drain(..).collect();
            batches.extend(self.inner.parse_arrow_batches(remaining, self.batch_size)?);
        }
        batches_to_ipc_bytes(batches)
    }
}

#[wasm_bindgen]
pub struct StreamObjectTjsonReaderHandle {
    scanner: TjsonArrayScanner,
    schema: StreamSchema,
    mode: ParseMode,
}

#[wasm_bindgen]
impl StreamObjectTjsonReaderHandle {
    #[wasm_bindgen(constructor)]
    pub fn new(schema_json: &str, mode: &str) -> Result<StreamObjectTjsonReaderHandle, JsError> {
        Ok(Self {
            scanner: TjsonArrayScanner::new(),
            schema: schema_from_json(schema_json)?,
            mode: parse_mode(mode),
        })
    }

    pub fn feed(&mut self, chunk: &str) -> Result<String, JsError> {
        let objects = self.scanner.feed_objects(chunk, false)?;
        rows_to_wire_json(self.scanner.parse_rows(objects, &self.schema, self.mode)?)
    }

    pub fn finish(&mut self) -> Result<String, JsError> {
        let objects = self.scanner.feed_objects("", true)?;
        rows_to_wire_json(self.scanner.parse_rows(objects, &self.schema, self.mode)?)
    }
}

#[wasm_bindgen]
pub struct StreamArrowTjsonReaderHandle {
    scanner: TjsonArrayScanner,
    schema: StreamSchema,
    mode: ParseMode,
    batch_size: usize,
    obj_buffer: Vec<String>,
}

impl StreamArrowTjsonReaderHandle {
    fn flush_full_batches(&mut self) -> Result<Vec<RecordBatch>, JsError> {
        let mut all_batches = Vec::new();
        while self.obj_buffer.len() >= self.batch_size {
            let chunk: Vec<String> = self.obj_buffer.drain(..self.batch_size).collect();
            all_batches.extend(self.scanner.parse_arrow_batches(
                chunk,
                &self.schema,
                self.batch_size,
                self.mode,
            )?);
        }
        Ok(all_batches)
    }
}

#[wasm_bindgen]
impl StreamArrowTjsonReaderHandle {
    #[wasm_bindgen(constructor)]
    pub fn new(
        schema_json: &str,
        batch_size: usize,
        mode: &str,
    ) -> Result<StreamArrowTjsonReaderHandle, JsError> {
        Ok(Self {
            scanner: TjsonArrayScanner::new(),
            schema: schema_from_json(schema_json)?,
            mode: parse_mode(mode),
            batch_size,
            obj_buffer: Vec::new(),
        })
    }

    pub fn feed(&mut self, chunk: &str) -> Result<Vec<u8>, JsError> {
        let objects = self.scanner.feed_objects(chunk, false)?;
        self.obj_buffer.extend(objects);
        batches_to_ipc_bytes(self.flush_full_batches()?)
    }

    pub fn finish(&mut self) -> Result<Vec<u8>, JsError> {
        let objects = self.scanner.feed_objects("", true)?;
        self.obj_buffer.extend(objects);
        let mut batches = self.flush_full_batches()?;
        if !self.obj_buffer.is_empty() {
            let remaining: Vec<String> = self.obj_buffer.drain(..).collect();
            batches.extend(self.scanner.parse_arrow_batches(
                remaining,
                &self.schema,
                self.batch_size,
                self.mode,
            )?);
        }
        batches_to_ipc_bytes(batches)
    }
}

#[wasm_bindgen]
pub struct StreamObjectWriterHandle {
    writer: StreamWriter<SharedTextSink>,
    sink: SharedTextSink,
    emitted_offset: usize,
    rows_emitted: usize,
}

#[wasm_bindgen]
impl StreamObjectWriterHandle {
    #[wasm_bindgen(constructor)]
    pub fn new(schema_json: &str, opts_json: &str) -> Result<StreamObjectWriterHandle, JsError> {
        let schema = schema_from_json(schema_json)?;
        let sink = SharedTextSink::default();
        let writer = StreamWriter::new(sink.clone(), schema, parse_ttoon_opts(opts_json));
        Ok(Self {
            writer,
            sink,
            emitted_offset: 0,
            rows_emitted: 0,
        })
    }

    pub fn write_row(&mut self, row_json: &str) -> Result<String, JsError> {
        let row = row_json_to_map(row_json)?;
        self.writer.write(&row).map_err(ttoon_err)?;
        self.rows_emitted += 1;
        self.sink.take_text_since(&mut self.emitted_offset)
    }

    pub fn close(&mut self) -> Result<String, JsError> {
        self.writer.close().map_err(ttoon_err)?;
        self.sink.take_text_since(&mut self.emitted_offset)
    }

    #[wasm_bindgen(getter)]
    pub fn rows_emitted(&self) -> usize {
        self.rows_emitted
    }
}

#[wasm_bindgen]
pub struct StreamArrowWriterHandle {
    writer: ArrowStreamWriter<SharedTextSink>,
    sink: SharedTextSink,
    emitted_offset: usize,
    rows_emitted: usize,
}

#[wasm_bindgen]
impl StreamArrowWriterHandle {
    #[wasm_bindgen(constructor)]
    pub fn new(schema_json: &str, opts_json: &str) -> Result<StreamArrowWriterHandle, JsError> {
        let schema = schema_from_json(schema_json)?;
        let sink = SharedTextSink::default();
        let writer = ArrowStreamWriter::new(sink.clone(), schema, parse_ttoon_opts(opts_json))
            .map_err(ttoon_err)?;
        Ok(Self {
            writer,
            sink,
            emitted_offset: 0,
            rows_emitted: 0,
        })
    }

    pub fn write_ipc(&mut self, ipc_bytes: &[u8]) -> Result<String, JsError> {
        let reader = IpcStreamReader::try_new(Cursor::new(ipc_bytes), None)
            .map_err(|err| JsError::new(&err.to_string()))?;
        for batch_result in reader {
            let batch = batch_result.map_err(|err| JsError::new(&err.to_string()))?;
            self.rows_emitted += batch.num_rows();
            self.writer.write_batch(&batch).map_err(ttoon_err)?;
        }
        self.sink.take_text_since(&mut self.emitted_offset)
    }

    pub fn close(&mut self) -> Result<String, JsError> {
        self.writer.close().map_err(ttoon_err)?;
        self.sink.take_text_since(&mut self.emitted_offset)
    }

    #[wasm_bindgen(getter)]
    pub fn rows_emitted(&self) -> usize {
        self.rows_emitted
    }
}

#[wasm_bindgen]
pub struct StreamObjectTjsonWriterHandle {
    writer: TjsonStreamWriter<SharedTextSink>,
    sink: SharedTextSink,
    emitted_offset: usize,
    rows_emitted: usize,
}

#[wasm_bindgen]
impl StreamObjectTjsonWriterHandle {
    #[wasm_bindgen(constructor)]
    pub fn new(
        schema_json: &str,
        opts_json: &str,
    ) -> Result<StreamObjectTjsonWriterHandle, JsError> {
        let schema = schema_from_json(schema_json)?;
        let sink = SharedTextSink::default();
        let writer = TjsonStreamWriter::new(sink.clone(), schema, parse_tjson_opts(opts_json));
        Ok(Self {
            writer,
            sink,
            emitted_offset: 0,
            rows_emitted: 0,
        })
    }

    pub fn write_row(&mut self, row_json: &str) -> Result<String, JsError> {
        let row = row_json_to_map(row_json)?;
        self.writer.write(&row).map_err(ttoon_err)?;
        self.rows_emitted += 1;
        self.sink.take_text_since(&mut self.emitted_offset)
    }

    pub fn close(&mut self) -> Result<String, JsError> {
        self.writer.close().map_err(ttoon_err)?;
        self.sink.take_text_since(&mut self.emitted_offset)
    }

    #[wasm_bindgen(getter)]
    pub fn rows_emitted(&self) -> usize {
        self.rows_emitted
    }
}

#[wasm_bindgen]
pub struct StreamArrowTjsonWriterHandle {
    writer: TjsonArrowStreamWriter<SharedTextSink>,
    sink: SharedTextSink,
    emitted_offset: usize,
    rows_emitted: usize,
}

#[wasm_bindgen]
impl StreamArrowTjsonWriterHandle {
    #[wasm_bindgen(constructor)]
    pub fn new(
        schema_json: &str,
        opts_json: &str,
    ) -> Result<StreamArrowTjsonWriterHandle, JsError> {
        let schema = schema_from_json(schema_json)?;
        let sink = SharedTextSink::default();
        let writer = TjsonArrowStreamWriter::new(sink.clone(), schema, parse_tjson_opts(opts_json))
            .map_err(ttoon_err)?;
        Ok(Self {
            writer,
            sink,
            emitted_offset: 0,
            rows_emitted: 0,
        })
    }

    pub fn write_ipc(&mut self, ipc_bytes: &[u8]) -> Result<String, JsError> {
        let reader = IpcStreamReader::try_new(Cursor::new(ipc_bytes), None)
            .map_err(|err| JsError::new(&err.to_string()))?;
        for batch_result in reader {
            let batch = batch_result.map_err(|err| JsError::new(&err.to_string()))?;
            self.rows_emitted += batch.num_rows();
            self.writer.write_batch(&batch).map_err(ttoon_err)?;
        }
        self.sink.take_text_since(&mut self.emitted_offset)
    }

    pub fn close(&mut self) -> Result<String, JsError> {
        self.writer.close().map_err(ttoon_err)?;
        self.sink.take_text_since(&mut self.emitted_offset)
    }

    #[wasm_bindgen(getter)]
    pub fn rows_emitted(&self) -> usize {
        self.rows_emitted
    }
}
