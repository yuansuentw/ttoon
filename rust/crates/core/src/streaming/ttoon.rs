use std::io::BufRead;

use arrow_array::RecordBatch;
use indexmap::IndexMap;

use crate::ir::Node;
use crate::ttoon_parser::{
    parse_root_tabular_header, split_row_quote_aware, TabularHeader, TabularRowCount,
};
use crate::typed_value::ParsedTypedValue;
use crate::{Error, ErrorKind, ParseMode, Result, StreamSchema};

use super::core::{
    convert_typed_value_to_arrow_value, io_error, validate_typed_value, ArrowBatchSink,
};

pub struct StreamReader<R: BufRead> {
    source: R,
    schema: StreamSchema,
    mode: ParseMode,
    initialized: bool,
    delimiter: crate::Delimiter,
    row_count: TabularRowCount,
    rows_read: usize,
    finished: bool,
}

impl<R: BufRead> StreamReader<R> {
    pub fn new(source: R, schema: StreamSchema) -> Self {
        Self::with_mode(source, schema, ParseMode::Compat)
    }

    pub fn with_mode(source: R, schema: StreamSchema, mode: ParseMode) -> Self {
        Self {
            source,
            schema,
            mode,
            initialized: false,
            delimiter: crate::Delimiter::Comma,
            row_count: TabularRowCount::Exact(0),
            rows_read: 0,
            finished: false,
        }
    }

    fn initialize(&mut self) -> Result<()> {
        let Some(header_line) = self.read_next_non_empty_line()? else {
            return Err(Error::new(
                ErrorKind::ParseError,
                "stream_read: missing tabular header",
                None,
            ));
        };

        let header = parse_root_tabular_header(&header_line)?;
        self.validate_header(&header)?;
        self.delimiter = header.delimiter;
        self.row_count = header.row_count;
        self.initialized = true;

        if matches!(self.row_count, TabularRowCount::Exact(0)) {
            if self.read_next_non_empty_line()?.is_some() {
                return Err(Error::new(
                    ErrorKind::ParseError,
                    "stream_read: header declares 0 rows but trailing data was found",
                    None,
                ));
            }
            self.finished = true;
        }

        Ok(())
    }

    fn validate_header(&self, header: &TabularHeader) -> Result<()> {
        if header.fields.len() != self.schema.len() {
            return Err(Error::new(
                ErrorKind::ParseError,
                format!(
                    "stream_read: header has {} fields, schema expects {}",
                    header.fields.len(),
                    self.schema.len()
                ),
                None,
            ));
        }

        for (expected, actual) in self.schema.fields().iter().zip(header.fields.iter()) {
            if expected.name() != actual {
                return Err(Error::new(
                    ErrorKind::ParseError,
                    format!(
                        "stream_read: header field '{}' does not match schema field '{}'",
                        actual,
                        expected.name()
                    ),
                    None,
                ));
            }
        }

        Ok(())
    }

    fn read_next_non_empty_line(&mut self) -> Result<Option<String>> {
        let mut buf = String::new();
        loop {
            buf.clear();
            let read = self
                .source
                .read_line(&mut buf)
                .map_err(|err| io_error(ErrorKind::ParseError, "stream_read", err))?;
            if read == 0 {
                return Ok(None);
            }

            let trimmed = if self.initialized {
                buf.trim()
            } else {
                buf.trim_start_matches('\u{FEFF}').trim()
            };
            if trimmed.is_empty() {
                continue;
            }
            return Ok(Some(trimmed.to_string()));
        }
    }
}

impl<R: BufRead> Iterator for StreamReader<R> {
    type Item = Result<IndexMap<String, Node>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        if !self.initialized {
            if let Err(err) = self.initialize() {
                self.finished = true;
                return Some(Err(err));
            }
            if self.finished {
                return None;
            }
        }

        let line = match self.read_next_non_empty_line() {
            Ok(Some(line)) => line,
            Ok(None) => {
                self.finished = true;
                return match self.row_count {
                    TabularRowCount::Streaming if self.rows_read == 0 => Some(Err(Error::new(
                        ErrorKind::ParseError,
                        "stream_read: [*] header requires at least one data row",
                        None,
                    ))),
                    TabularRowCount::Exact(expected) if self.rows_read < expected => {
                        Some(Err(Error::new(
                            ErrorKind::ParseError,
                            format!(
                                "stream_read: expected {} rows, got {}",
                                expected, self.rows_read
                            ),
                            None,
                        )))
                    }
                    _ => None,
                };
            }
            Err(err) => {
                self.finished = true;
                return Some(Err(err));
            }
        };

        if let TabularRowCount::Exact(expected) = self.row_count {
            if self.rows_read >= expected {
                self.finished = true;
                return Some(Err(Error::new(
                    ErrorKind::ParseError,
                    format!(
                        "stream_read: expected {} rows, but trailing data was found",
                        expected
                    ),
                    None,
                )));
            }
        }

        let cells = match split_row_quote_aware(&line, &self.delimiter) {
            Ok(cells) => cells,
            Err(err) => {
                self.finished = true;
                return Some(Err(err));
            }
        };

        if cells.len() != self.schema.len() {
            self.finished = true;
            return Some(Err(Error::new(
                ErrorKind::ParseError,
                format!(
                    "stream_read: row has {} fields, schema expects {}",
                    cells.len(),
                    self.schema.len()
                ),
                None,
            )));
        }

        let mut row = IndexMap::new();
        for (field, raw) in self.schema.fields().iter().zip(cells.iter()) {
            let typed_value = match parse_ttoon_typed_value(raw.trim(), self.mode) {
                Ok(value) => value,
                Err(err) => {
                    self.finished = true;
                    return Some(Err(err));
                }
            };
            if let Err(err) =
                validate_typed_value(field, &typed_value, ErrorKind::ParseError, "stream_read")
            {
                self.finished = true;
                return Some(Err(err));
            }
            row.insert(field.name().to_string(), typed_value.into());
        }

        self.rows_read += 1;
        Some(Ok(row))
    }
}

pub struct ArrowStreamReader<R: BufRead> {
    source: R,
    sink: ArrowBatchSink,
    mode: ParseMode,
    initialized: bool,
    delimiter: crate::Delimiter,
    row_count: TabularRowCount,
    rows_read: usize,
    finished: bool,
    pending_error: Option<Error>,
}

impl<R: BufRead> ArrowStreamReader<R> {
    pub fn new(source: R, schema: StreamSchema, batch_size: usize) -> Result<Self> {
        Self::with_mode(source, schema, batch_size, ParseMode::Compat)
    }

    pub fn with_mode(
        source: R,
        schema: StreamSchema,
        batch_size: usize,
        mode: ParseMode,
    ) -> Result<Self> {
        Ok(Self {
            source,
            sink: ArrowBatchSink::new(schema, batch_size, "stream_read_arrow")?,
            mode,
            initialized: false,
            delimiter: crate::Delimiter::Comma,
            row_count: TabularRowCount::Exact(0),
            rows_read: 0,
            finished: false,
            pending_error: None,
        })
    }

    fn initialize(&mut self) -> Result<()> {
        let Some(header_line) = self.read_next_non_empty_line()? else {
            return Err(Error::new(
                ErrorKind::ParseError,
                "stream_read_arrow: missing tabular header",
                None,
            ));
        };

        let header = parse_root_tabular_header(&header_line)?;
        self.validate_header(&header)?;
        self.delimiter = header.delimiter;
        self.row_count = header.row_count;
        self.initialized = true;

        if matches!(self.row_count, TabularRowCount::Exact(0)) {
            if self.read_next_non_empty_line()?.is_some() {
                return Err(Error::new(
                    ErrorKind::ParseError,
                    "stream_read_arrow: header declares 0 rows but trailing data was found",
                    None,
                ));
            }
            self.finished = true;
        }

        Ok(())
    }

    fn validate_header(&self, header: &TabularHeader) -> Result<()> {
        if header.fields.len() != self.sink.schema().len() {
            return Err(Error::new(
                ErrorKind::ParseError,
                format!(
                    "stream_read_arrow: header has {} fields, schema expects {}",
                    header.fields.len(),
                    self.sink.schema().len()
                ),
                None,
            ));
        }

        for (expected, actual) in self.sink.schema().fields().iter().zip(header.fields.iter()) {
            if expected.name() != actual {
                return Err(Error::new(
                    ErrorKind::ParseError,
                    format!(
                        "stream_read_arrow: header field '{}' does not match schema field '{}'",
                        actual,
                        expected.name()
                    ),
                    None,
                ));
            }
        }

        Ok(())
    }

    fn read_next_non_empty_line(&mut self) -> Result<Option<String>> {
        let mut buf = String::new();
        loop {
            buf.clear();
            let read = self
                .source
                .read_line(&mut buf)
                .map_err(|err| io_error(ErrorKind::ParseError, "stream_read_arrow", err))?;
            if read == 0 {
                return Ok(None);
            }

            let trimmed = if self.initialized {
                buf.trim()
            } else {
                buf.trim_start_matches('\u{FEFF}').trim()
            };
            if trimmed.is_empty() {
                continue;
            }
            return Ok(Some(trimmed.to_string()));
        }
    }

    fn append_line(&mut self, line: &str) -> Result<()> {
        let cells = split_row_quote_aware(line, &self.delimiter)?;
        if cells.len() != self.sink.schema().len() {
            return Err(Error::new(
                ErrorKind::ParseError,
                format!(
                    "stream_read_arrow: row has {} fields, schema expects {}",
                    cells.len(),
                    self.sink.schema().len()
                ),
                None,
            ));
        }

        let mut values = Vec::with_capacity(cells.len());
        for (field, raw) in self.sink.schema().fields().iter().zip(cells.iter()) {
            let typed_value = parse_ttoon_typed_value(raw.trim(), self.mode)?;
            values.push(convert_typed_value_to_arrow_value(
                field,
                typed_value,
                "stream_read_arrow",
            )?);
        }

        self.sink.append_row(values)?;
        self.rows_read += 1;
        Ok(())
    }

    fn queue_or_return_error(&mut self, err: Error) -> Option<Result<RecordBatch>> {
        self.finished = true;
        if self.sink.has_buffered_rows() {
            self.pending_error = Some(err);
            return Some(self.sink.flush_batch());
        }
        Some(Err(err))
    }
}

impl<R: BufRead> Iterator for ArrowStreamReader<R> {
    type Item = Result<RecordBatch>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(err) = self.pending_error.take() {
            return Some(Err(err));
        }

        if self.finished {
            return None;
        }

        if !self.initialized {
            if let Err(err) = self.initialize() {
                self.finished = true;
                return Some(Err(err));
            }
            if self.finished {
                return None;
            }
        }

        loop {
            if self.sink.rows_buffered() >= self.sink.batch_size() {
                return Some(self.sink.flush_batch());
            }

            let line = match self.read_next_non_empty_line() {
                Ok(Some(line)) => line,
                Ok(None) => {
                    self.finished = true;
                    let eof_error = match self.row_count {
                        TabularRowCount::Streaming if self.rows_read == 0 => Some(Error::new(
                            ErrorKind::ParseError,
                            "stream_read_arrow: [*] header requires at least one data row",
                            None,
                        )),
                        TabularRowCount::Exact(expected) if self.rows_read < expected => {
                            Some(Error::new(
                                ErrorKind::ParseError,
                                format!(
                                    "stream_read_arrow: expected {} rows, got {}",
                                    expected, self.rows_read
                                ),
                                None,
                            ))
                        }
                        _ => None,
                    };

                    if let Some(err) = eof_error {
                        return self.queue_or_return_error(err);
                    }

                    if self.sink.has_buffered_rows() {
                        return Some(self.sink.flush_batch());
                    }

                    return None;
                }
                Err(err) => return self.queue_or_return_error(err),
            };

            if let TabularRowCount::Exact(expected) = self.row_count {
                if self.rows_read >= expected {
                    return self.queue_or_return_error(Error::new(
                        ErrorKind::ParseError,
                        format!(
                            "stream_read_arrow: expected {} rows, but trailing data was found",
                            expected
                        ),
                        None,
                    ));
                }
            }

            if let Err(err) = self.append_line(&line) {
                return self.queue_or_return_error(err);
            }
        }
    }
}

fn parse_ttoon_typed_value(raw: &str, mode: ParseMode) -> Result<ParsedTypedValue> {
    crate::typed_parse::parse_unit_typed_value(raw, mode)
}
