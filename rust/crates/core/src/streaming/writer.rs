use std::io::Write;
use std::sync::Arc;

use arrow_array::{Array, RecordBatch};
use arrow_schema::Schema as ArrowSchema;
use indexmap::IndexMap;

use crate::arrow::{format_arrow_value_to_tjson_buffer, format_arrow_value_to_ttoon_buffer};
use crate::ir::Node;
use crate::ttoon_parser::{format_root_tabular_header, TabularRowCount};
use crate::ttoon_serializer;
use crate::{Error, ErrorKind, Result, StreamSchema, TjsonOptions, TtoonOptions};

use super::core::{
    io_error, validate_field_value, validate_record_batch_schema, validate_row_keys, StreamResult,
};

pub struct StreamWriter<W: Write> {
    sink: W,
    schema: StreamSchema,
    opts: TtoonOptions,
    header_written: bool,
    rows_emitted: usize,
    closed: bool,
}

impl<W: Write> StreamWriter<W> {
    pub fn new(sink: W, schema: StreamSchema, opts: TtoonOptions) -> Self {
        Self {
            sink,
            schema,
            opts,
            header_written: false,
            rows_emitted: 0,
            closed: false,
        }
    }

    pub fn write(&mut self, row: &IndexMap<String, Node>) -> Result<()> {
        if self.closed {
            return Err(Error::new(
                ErrorKind::SerializeError,
                "stream_write: writer is already closed",
                None,
            ));
        }

        validate_row_keys(row, &self.schema, ErrorKind::SerializeError, "stream_write")?;

        let mut row_buf = String::with_capacity(128);
        let delim_cell = ttoon_serializer::delim_cell(self.opts.delimiter);
        for (index, field) in self.schema.fields().iter().enumerate() {
            if index > 0 {
                row_buf.push_str(delim_cell);
            }
            let value = row.get(field.name()).expect("row keys already validated");
            validate_field_value(field, value, ErrorKind::SerializeError, "stream_write")?;
            ttoon_serializer::write_scalar(value, &self.opts, &mut row_buf)?;
        }
        row_buf.push('\n');

        let mut chunk = String::with_capacity(row_buf.len() + 64);
        if !self.header_written {
            chunk.push_str(&format_root_tabular_header(
                TabularRowCount::Streaming,
                &self.schema.field_names(),
                self.opts.delimiter,
            ));
            chunk.push('\n');
        }
        chunk.push_str(&row_buf);

        self.sink
            .write_all(chunk.as_bytes())
            .map_err(|err| io_error(ErrorKind::SerializeError, "stream_write", err))?;

        self.header_written = true;
        self.rows_emitted += 1;
        Ok(())
    }

    pub fn close(&mut self) -> Result<StreamResult> {
        if self.closed {
            return Err(Error::new(
                ErrorKind::SerializeError,
                "stream_write: writer is already closed",
                None,
            ));
        }

        if !self.header_written {
            let mut chunk = format_root_tabular_header(
                TabularRowCount::Exact(0),
                &self.schema.field_names(),
                self.opts.delimiter,
            );
            chunk.push('\n');
            self.sink
                .write_all(chunk.as_bytes())
                .map_err(|err| io_error(ErrorKind::SerializeError, "stream_write", err))?;
            self.header_written = true;
        }

        self.sink
            .flush()
            .map_err(|err| io_error(ErrorKind::SerializeError, "stream_write", err))?;
        self.closed = true;

        Ok(StreamResult {
            rows_emitted: self.rows_emitted,
        })
    }
}

pub struct ArrowStreamWriter<W: Write> {
    sink: W,
    schema: StreamSchema,
    arrow_schema: Arc<ArrowSchema>,
    opts: TtoonOptions,
    streaming_header: String,
    empty_header: String,
    delim_cell: &'static str,
    header_written: bool,
    rows_emitted: usize,
    closed: bool,
}

impl<W: Write> ArrowStreamWriter<W> {
    pub fn new(sink: W, schema: StreamSchema, opts: TtoonOptions) -> Result<Self> {
        let arrow_schema = Arc::new(schema.to_arrow_schema()?);
        let field_names = schema.field_names();
        let streaming_header =
            format_root_tabular_header(TabularRowCount::Streaming, &field_names, opts.delimiter);
        let empty_header =
            format_root_tabular_header(TabularRowCount::Exact(0), &field_names, opts.delimiter);
        let delim_cell = ttoon_serializer::delim_cell(opts.delimiter);
        Ok(Self {
            sink,
            schema,
            arrow_schema,
            opts,
            streaming_header,
            empty_header,
            delim_cell,
            header_written: false,
            rows_emitted: 0,
            closed: false,
        })
    }

    pub fn write_batch(&mut self, batch: &RecordBatch) -> Result<()> {
        if self.closed {
            return Err(Error::new(
                ErrorKind::SerializeError,
                "stream_write_arrow: writer is already closed",
                None,
            ));
        }

        validate_record_batch_schema(batch, self.arrow_schema.fields(), "stream_write_arrow")?;

        let num_rows = batch.num_rows();
        if num_rows == 0 {
            return Ok(());
        }

        let mut chunk = String::with_capacity(num_rows.saturating_mul(64) + 64);
        if !self.header_written {
            chunk.push_str(&self.streaming_header);
            chunk.push('\n');
        }

        let batch_columns = batch.columns();
        let schema_fields = self.arrow_schema.fields();
        let stream_fields = self.schema.fields();
        for row_idx in 0..num_rows {
            for (col_idx, field) in schema_fields.iter().enumerate() {
                if col_idx > 0 {
                    chunk.push_str(self.delim_cell);
                }
                let column = &batch_columns[col_idx];
                let stream_field = &stream_fields[col_idx];
                if column.is_null(row_idx) && !stream_field.field_type().is_nullable() {
                    return Err(Error::new(
                        ErrorKind::SerializeError,
                        format!(
                            "stream_write_arrow: field '{}' is not nullable",
                            stream_field.name()
                        ),
                        None,
                    ));
                }
                format_arrow_value_to_ttoon_buffer(
                    &mut chunk,
                    column.as_ref(),
                    row_idx,
                    field.as_ref(),
                    self.opts.binary_format,
                )?;
            }
            chunk.push('\n');
        }

        self.sink
            .write_all(chunk.as_bytes())
            .map_err(|err| io_error(ErrorKind::SerializeError, "stream_write_arrow", err))?;

        self.header_written = true;
        self.rows_emitted += num_rows;
        Ok(())
    }

    pub fn close(&mut self) -> Result<StreamResult> {
        if self.closed {
            return Err(Error::new(
                ErrorKind::SerializeError,
                "stream_write_arrow: writer is already closed",
                None,
            ));
        }

        if !self.header_written {
            let mut chunk = self.empty_header.clone();
            chunk.push('\n');
            self.sink
                .write_all(chunk.as_bytes())
                .map_err(|err| io_error(ErrorKind::SerializeError, "stream_write_arrow", err))?;
            self.header_written = true;
        }

        self.sink
            .flush()
            .map_err(|err| io_error(ErrorKind::SerializeError, "stream_write_arrow", err))?;
        self.closed = true;

        Ok(StreamResult {
            rows_emitted: self.rows_emitted,
        })
    }

    pub fn arrow_schema(&self) -> &ArrowSchema {
        &self.arrow_schema
    }
}

enum ArrayWriterState {
    Open,
    Failed,
    Closed,
}

struct JsonArrayWriterCore<W: Write> {
    sink: W,
    started: bool,
    rows_emitted: usize,
    state: ArrayWriterState,
}

impl<W: Write> JsonArrayWriterCore<W> {
    fn new(sink: W) -> Self {
        Self {
            sink,
            started: false,
            rows_emitted: 0,
            state: ArrayWriterState::Open,
        }
    }

    fn write_object_text(&mut self, object_text: &str, context: &str) -> Result<()> {
        self.ensure_writable(context)?;

        let prefix = if self.started { ", " } else { "[" };
        if let Err(err) = self.sink.write_all(prefix.as_bytes()) {
            self.enter_failed();
            return Err(io_error(ErrorKind::SerializeError, context, err));
        }
        if let Err(err) = self.sink.write_all(object_text.as_bytes()) {
            self.enter_failed();
            return Err(io_error(ErrorKind::SerializeError, context, err));
        }

        self.started = true;
        self.rows_emitted += 1;
        Ok(())
    }

    fn ensure_writable(&self, context: &str) -> Result<()> {
        match self.state {
            ArrayWriterState::Open => Ok(()),
            ArrayWriterState::Closed => Err(Error::new(
                ErrorKind::SerializeError,
                format!("{}: writer is already closed", context),
                None,
            )),
            ArrayWriterState::Failed => Err(Error::new(
                ErrorKind::SerializeError,
                format!("{}: writer is in failed state", context),
                None,
            )),
        }
    }

    fn enter_failed(&mut self) {
        if matches!(self.state, ArrayWriterState::Open) {
            self.state = ArrayWriterState::Failed;
        }
    }

    fn close(&mut self, context: &str) -> Result<StreamResult> {
        match self.state {
            ArrayWriterState::Closed => {
                return Err(Error::new(
                    ErrorKind::SerializeError,
                    format!("{}: writer is already closed", context),
                    None,
                ))
            }
            ArrayWriterState::Failed => {
                return Err(Error::new(
                    ErrorKind::SerializeError,
                    format!("{}: writer is in failed state", context),
                    None,
                ))
            }
            ArrayWriterState::Open => {}
        }

        let suffix = if self.started { "]" } else { "[]" };
        if let Err(err) = self.sink.write_all(suffix.as_bytes()) {
            self.enter_failed();
            return Err(io_error(ErrorKind::SerializeError, context, err));
        }
        if let Err(err) = self.sink.flush() {
            self.enter_failed();
            return Err(io_error(ErrorKind::SerializeError, context, err));
        }

        self.state = ArrayWriterState::Closed;
        Ok(StreamResult {
            rows_emitted: self.rows_emitted,
        })
    }
}

pub struct TjsonStreamWriter<W: Write> {
    array_core: JsonArrayWriterCore<W>,
    schema: StreamSchema,
    opts: TjsonOptions,
}

impl<W: Write> TjsonStreamWriter<W> {
    pub fn new(sink: W, schema: StreamSchema, opts: TjsonOptions) -> Self {
        Self {
            array_core: JsonArrayWriterCore::new(sink),
            schema,
            opts,
        }
    }

    pub fn write(&mut self, row: &IndexMap<String, Node>) -> Result<()> {
        self.array_core.ensure_writable("stream_write_tjson")?;
        let mut object_text = String::with_capacity(128);
        if let Err(err) = format_tjson_object_row(&self.schema, row, &self.opts, &mut object_text) {
            self.array_core.enter_failed();
            return Err(err);
        }
        self.array_core
            .write_object_text(&object_text, "stream_write_tjson")
    }

    pub fn close(&mut self) -> Result<StreamResult> {
        self.array_core.close("stream_write_tjson")
    }
}

pub struct TjsonArrowStreamWriter<W: Write> {
    array_core: JsonArrayWriterCore<W>,
    schema: StreamSchema,
    arrow_schema: Arc<ArrowSchema>,
    opts: TjsonOptions,
}

impl<W: Write> TjsonArrowStreamWriter<W> {
    pub fn new(sink: W, schema: StreamSchema, opts: TjsonOptions) -> Result<Self> {
        let arrow_schema = Arc::new(schema.to_arrow_schema()?);
        Ok(Self {
            array_core: JsonArrayWriterCore::new(sink),
            schema,
            arrow_schema,
            opts,
        })
    }

    pub fn write_batch(&mut self, batch: &RecordBatch) -> Result<()> {
        self.array_core
            .ensure_writable("stream_write_tjson_arrow")?;
        if let Err(err) = validate_record_batch_schema(
            batch,
            self.arrow_schema.fields(),
            "stream_write_tjson_arrow",
        ) {
            self.array_core.enter_failed();
            return Err(err);
        }

        if batch.num_rows() == 0 {
            return Ok(());
        }

        let mut object_text = String::with_capacity(128);
        for row_idx in 0..batch.num_rows() {
            if let Err(err) = format_tjson_object_from_arrow_row(
                batch,
                row_idx,
                &self.schema,
                self.arrow_schema.as_ref(),
                &self.opts,
                &mut object_text,
            ) {
                self.array_core.enter_failed();
                return Err(err);
            }
            if let Err(err) = self
                .array_core
                .write_object_text(&object_text, "stream_write_tjson_arrow")
            {
                return Err(err);
            }
        }

        Ok(())
    }

    pub fn close(&mut self) -> Result<StreamResult> {
        self.array_core.close("stream_write_tjson_arrow")
    }
}

fn format_tjson_object_row(
    schema: &StreamSchema,
    row: &IndexMap<String, Node>,
    opts: &TjsonOptions,
    buf: &mut String,
) -> Result<()> {
    validate_row_keys(row, schema, ErrorKind::SerializeError, "stream_write_tjson")?;

    buf.clear();
    buf.push('{');
    for (index, field) in schema.fields().iter().enumerate() {
        if index > 0 {
            buf.push_str(", ");
        }
        let value = row.get(field.name()).expect("row keys already validated");
        validate_field_value(
            field,
            value,
            ErrorKind::SerializeError,
            "stream_write_tjson",
        )?;

        buf.push('"');
        crate::typed_fmt::escape_tjson_string(buf, field.name());
        buf.push_str("\": ");
        crate::tjson_serializer::format_scalar_to_tjson_buffer(buf, value, opts)?;
    }
    buf.push('}');
    Ok(())
}

fn format_tjson_object_from_arrow_row(
    batch: &RecordBatch,
    row_idx: usize,
    schema: &StreamSchema,
    expected_schema: &ArrowSchema,
    opts: &TjsonOptions,
    buf: &mut String,
) -> Result<()> {
    buf.clear();
    buf.push('{');

    for (col_idx, field) in expected_schema.fields().iter().enumerate() {
        if col_idx > 0 {
            buf.push_str(", ");
        }

        let stream_field = &schema.fields()[col_idx];
        let column = batch.column(col_idx);
        if column.is_null(row_idx) && !stream_field.field_type().is_nullable() {
            return Err(Error::new(
                ErrorKind::SerializeError,
                format!(
                    "stream_write_tjson_arrow: field '{}' is not nullable",
                    stream_field.name()
                ),
                None,
            ));
        }

        buf.push('"');
        crate::typed_fmt::escape_tjson_string(buf, field.name());
        buf.push_str("\": ");
        format_arrow_value_to_tjson_buffer(
            buf,
            column.as_ref(),
            row_idx,
            field.as_ref(),
            opts.binary_format,
        )?;
    }

    buf.push('}');
    Ok(())
}
