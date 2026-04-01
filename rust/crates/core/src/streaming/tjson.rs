use std::collections::HashMap;
use std::io::BufRead;

use arrow_array::RecordBatch;
use indexmap::IndexMap;

use crate::ir::Node;
use crate::typed_value::ParsedTypedValue;
use crate::{Error, ErrorKind, ParseMode, Result, Span, StreamSchema};

use super::core::{convert_typed_value_to_arrow_value, validate_typed_value, ArrowBatchSink};

const OBJECT_CONTEXT: &str = "stream_read_tjson";
const ARROW_CONTEXT: &str = "stream_read_tjson_arrow";

pub struct TjsonStreamReader<R: BufRead> {
    scanner: ReaderStructuralScanner<R>,
    schema: StreamSchema,
    field_index: HashMap<String, usize>,
    mode: ParseMode,
    finished: bool,
}

impl<R: BufRead> TjsonStreamReader<R> {
    pub fn new(source: R, schema: StreamSchema) -> Self {
        Self::with_mode(source, schema, ParseMode::Compat)
    }

    pub fn with_mode(source: R, schema: StreamSchema, mode: ParseMode) -> Self {
        Self {
            scanner: ReaderStructuralScanner::new(source),
            field_index: build_field_index(&schema),
            schema,
            mode,
            finished: false,
        }
    }
}

impl<R: BufRead> Iterator for TjsonStreamReader<R> {
    type Item = Result<IndexMap<String, Node>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        let slots = match self.scanner.next_row_slots(
            &self.field_index,
            &self.schema,
            self.mode,
            OBJECT_CONTEXT,
        ) {
            Ok(Some(slots)) => slots,
            Ok(None) => {
                self.finished = true;
                return None;
            }
            Err(err) => {
                self.finished = true;
                return Some(Err(err));
            }
        };

        let mut row = IndexMap::new();
        for (field, slot) in self.schema.fields().iter().zip(slots.into_iter()) {
            let typed_value = slot.unwrap_or(ParsedTypedValue::Null);
            if let Err(err) =
                validate_typed_value(field, &typed_value, ErrorKind::ParseError, OBJECT_CONTEXT)
            {
                self.finished = true;
                return Some(Err(err));
            }
            row.insert(field.name().to_string(), typed_value.into());
        }

        Some(Ok(row))
    }
}

pub struct TjsonArrowStreamReader<R: BufRead> {
    scanner: ReaderStructuralScanner<R>,
    sink: ArrowBatchSink,
    field_index: HashMap<String, usize>,
    mode: ParseMode,
    rows_read: usize,
    finished: bool,
    pending_error: Option<Error>,
}

impl<R: BufRead> TjsonArrowStreamReader<R> {
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
            scanner: ReaderStructuralScanner::new(source),
            field_index: build_field_index(&schema),
            sink: ArrowBatchSink::new(schema, batch_size, ARROW_CONTEXT)?,
            mode,
            rows_read: 0,
            finished: false,
            pending_error: None,
        })
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

impl<R: BufRead> Iterator for TjsonArrowStreamReader<R> {
    type Item = Result<RecordBatch>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(err) = self.pending_error.take() {
            return Some(Err(err));
        }

        if self.finished {
            return None;
        }

        loop {
            if self.sink.rows_buffered() >= self.sink.batch_size() {
                return Some(self.sink.flush_batch());
            }

            let slots = match self.scanner.next_row_slots(
                &self.field_index,
                self.sink.schema(),
                self.mode,
                ARROW_CONTEXT,
            ) {
                Ok(Some(slots)) => slots,
                Ok(None) => {
                    self.finished = true;
                    if self.sink.has_buffered_rows() {
                        return Some(self.sink.flush_batch());
                    }
                    return None;
                }
                Err(err) => return self.queue_or_return_error(err),
            };

            let mut values = Vec::with_capacity(self.sink.schema().len());
            for (field, slot) in self.sink.schema().fields().iter().zip(slots.into_iter()) {
                let value = slot.unwrap_or(ParsedTypedValue::Null);
                match convert_typed_value_to_arrow_value(field, value, ARROW_CONTEXT) {
                    Ok(value) => values.push(value),
                    Err(err) => return self.queue_or_return_error(err),
                }
            }

            if let Err(err) = self.sink.append_row(values) {
                return self.queue_or_return_error(err);
            }
            self.rows_read += 1;
        }
    }
}

fn build_field_index(schema: &StreamSchema) -> HashMap<String, usize> {
    schema
        .fields()
        .iter()
        .enumerate()
        .map(|(index, field)| (field.name().to_string(), index))
        .collect()
}

struct ReaderStructuralScanner<R: BufRead> {
    source: R,
    offset: usize,
    line: usize,
    column: usize,
    initialized: bool,
    expect_delimiter_or_end: bool,
    finished: bool,
}

impl<R: BufRead> ReaderStructuralScanner<R> {
    fn new(source: R) -> Self {
        Self {
            source,
            offset: 0,
            line: 1,
            column: 1,
            initialized: false,
            expect_delimiter_or_end: false,
            finished: false,
        }
    }

    fn next_row_slots(
        &mut self,
        field_index: &HashMap<String, usize>,
        schema: &StreamSchema,
        mode: ParseMode,
        context: &str,
    ) -> Result<Option<Vec<Option<ParsedTypedValue>>>> {
        if self.finished {
            return Ok(None);
        }

        if !self.initialized {
            self.skip_insignificant()?;
            self.expect_byte(b'[', context, "top-level value must be array")?;
            self.initialized = true;
        }

        if self.expect_delimiter_or_end {
            self.skip_insignificant()?;
            match self.peek_byte()? {
                Some(b',') => {
                    self.bump_byte()?;
                    self.skip_insignificant()?;
                    if matches!(self.peek_byte()?, Some(b']')) {
                        return Err(Error::new(
                            ErrorKind::ParseError,
                            format!("{}: expected object after ','", context),
                            Some(self.current_span()),
                        ));
                    }
                }
                Some(b']') => {
                    self.bump_byte()?;
                    self.finish_array(context)?;
                    return Ok(None);
                }
                Some(_) => {
                    return Err(Error::new(
                        ErrorKind::ParseError,
                        format!("{}: expected ',' or ']'", context),
                        Some(self.current_span()),
                    ))
                }
                None => {
                    return Err(Error::new(
                        ErrorKind::ParseError,
                        format!("{}: unexpected end of input", context),
                        Some(self.current_span()),
                    ))
                }
            }
        }

        self.skip_insignificant()?;
        match self.peek_byte()? {
            Some(b']') => {
                self.bump_byte()?;
                self.finish_array(context)?;
                Ok(None)
            }
            Some(b'{') => {
                let slots = self.read_object_slots(field_index, schema, mode, context)?;
                self.expect_delimiter_or_end = true;
                Ok(Some(slots))
            }
            Some(_) => Err(Error::new(
                ErrorKind::ParseError,
                format!("{}: array items must be objects", context),
                Some(self.current_span()),
            )),
            None => Err(Error::new(
                ErrorKind::ParseError,
                format!("{}: unexpected end of input", context),
                Some(self.current_span()),
            )),
        }
    }

    fn finish_array(&mut self, context: &str) -> Result<()> {
        self.skip_insignificant()?;
        if self.peek_byte()?.is_some() {
            return Err(Error::new(
                ErrorKind::ParseError,
                format!("{}: unexpected trailing data after array", context),
                Some(self.current_span()),
            ));
        }
        self.finished = true;
        Ok(())
    }

    fn read_object_slots(
        &mut self,
        field_index: &HashMap<String, usize>,
        schema: &StreamSchema,
        mode: ParseMode,
        context: &str,
    ) -> Result<Vec<Option<ParsedTypedValue>>> {
        self.expect_byte(b'{', context, "expected object")?;
        self.skip_insignificant()?;

        let mut slots = vec![None; schema.len()];
        if matches!(self.peek_byte()?, Some(b'}')) {
            self.bump_byte()?;
            return Ok(slots);
        }

        loop {
            let key_span = self.current_span();
            let raw_key = self.read_json_string_inner(context)?;
            let key = crate::typed_parse::unescape_tjson_string(&raw_key)
                .map_err(|err| Error::new(ErrorKind::LexError, err.message, Some(key_span)))?;

            self.skip_insignificant()?;
            self.expect_byte(b':', context, "expected ':' after object key")?;
            self.skip_insignificant()?;

            match field_index.get(&key).copied() {
                Some(index) => {
                    let value = self.read_known_scalar_value(context, &schema.fields()[index])?;
                    slots[index] = Some(value);
                }
                None => match mode {
                    ParseMode::Strict => {
                        return Err(Error::new(
                            ErrorKind::ParseError,
                            format!("{}: unexpected field '{}'", context, key),
                            Some(key_span),
                        ))
                    }
                    ParseMode::Compat => self.skip_any_value(context)?,
                },
            }

            self.skip_insignificant()?;
            match self.peek_byte()? {
                Some(b',') => {
                    self.bump_byte()?;
                    self.skip_insignificant()?;
                    if matches!(self.peek_byte()?, Some(b'}')) {
                        return Err(Error::new(
                            ErrorKind::ParseError,
                            format!("{}: expected value after ','", context),
                            Some(self.current_span()),
                        ));
                    }
                }
                Some(b'}') => {
                    self.bump_byte()?;
                    break;
                }
                Some(_) => {
                    return Err(Error::new(
                        ErrorKind::ParseError,
                        format!("{}: expected ',' or '}}' in object", context),
                        Some(self.current_span()),
                    ))
                }
                None => {
                    return Err(Error::new(
                        ErrorKind::ParseError,
                        format!("{}: unterminated object", context),
                        Some(self.current_span()),
                    ))
                }
            }
        }

        Ok(slots)
    }

    fn read_known_scalar_value(
        &mut self,
        context: &str,
        field: &crate::StreamField,
    ) -> Result<ParsedTypedValue> {
        let span = self.current_span();
        match self.peek_byte()? {
            Some(b'{') | Some(b'[') => {
                return Err(Error::new(
                    ErrorKind::ParseError,
                    format!(
                        "{}: field '{}' contains non-scalar value",
                        context,
                        field.name()
                    ),
                    Some(span),
                ))
            }
            Some(_) => {}
            None => {
                return Err(Error::new(
                    ErrorKind::ParseError,
                    format!("{}: unexpected end of input", context),
                    Some(span),
                ))
            }
        }

        let raw = self.read_scalar_value_raw(context)?;
        parse_tjson_scalar_raw(&raw, span)
    }

    fn skip_any_value(&mut self, context: &str) -> Result<()> {
        self.skip_insignificant()?;
        match self.peek_byte()? {
            Some(b'"') => {
                self.read_json_string_inner(context)?;
                Ok(())
            }
            Some(b'{') | Some(b'[') => self.skip_nested_value(context),
            Some(_) => {
                self.read_scalar_value_raw(context)?;
                Ok(())
            }
            None => Err(Error::new(
                ErrorKind::ParseError,
                format!("{}: unexpected end of input", context),
                Some(self.current_span()),
            )),
        }
    }

    fn skip_nested_value(&mut self, context: &str) -> Result<()> {
        let first = self.bump_byte()?.ok_or_else(|| {
            Error::new(
                ErrorKind::ParseError,
                format!("{}: unexpected end of input", context),
                Some(self.current_span()),
            )
        })?;
        let mut stack = vec![first];

        while let Some(byte) = self.bump_byte()? {
            match byte {
                b'"' => {
                    self.skip_string_body(context)?;
                }
                b'{' | b'[' => stack.push(byte),
                b'}' => {
                    if stack.pop() != Some(b'{') {
                        return Err(Error::new(
                            ErrorKind::ParseError,
                            format!("{}: invalid nested object structure", context),
                            Some(self.current_span()),
                        ));
                    }
                    if stack.is_empty() {
                        return Ok(());
                    }
                }
                b']' => {
                    if stack.pop() != Some(b'[') {
                        return Err(Error::new(
                            ErrorKind::ParseError,
                            format!("{}: invalid nested array structure", context),
                            Some(self.current_span()),
                        ));
                    }
                    if stack.is_empty() {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }

        Err(Error::new(
            ErrorKind::ParseError,
            format!("{}: unexpected end of input", context),
            Some(self.current_span()),
        ))
    }

    fn read_scalar_value_raw(&mut self, context: &str) -> Result<String> {
        self.skip_insignificant()?;
        let span = self.current_span();
        match self.peek_byte()? {
            Some(b'"') => {
                let inner = self.read_json_string_inner(context)?;
                let mut raw = String::with_capacity(inner.len() + 2);
                raw.push('"');
                raw.push_str(&inner);
                raw.push('"');
                Ok(raw)
            }
            Some(b',') | Some(b'}') | Some(b']') => Err(Error::new(
                ErrorKind::ParseError,
                format!("{}: missing value", context),
                Some(span),
            )),
            Some(_) => {
                let mut bytes = Vec::with_capacity(32);
                while let Some(byte) = self.peek_byte()? {
                    if byte.is_ascii_whitespace() || matches!(byte, b',' | b'}' | b']') {
                        break;
                    }
                    bytes.push(self.bump_byte()?.expect("peeked byte missing"));
                }
                if bytes.is_empty() {
                    return Err(Error::new(
                        ErrorKind::ParseError,
                        format!("{}: missing value", context),
                        Some(span),
                    ));
                }
                bytes_to_string(bytes, span)
            }
            None => Err(Error::new(
                ErrorKind::ParseError,
                format!("{}: unexpected end of input", context),
                Some(span),
            )),
        }
    }

    fn read_json_string_inner(&mut self, context: &str) -> Result<String> {
        let start_span = self.current_span();
        self.expect_byte(b'"', context, "expected string")?;

        let mut bytes = Vec::with_capacity(32);
        loop {
            let Some(byte) = self.bump_byte()? else {
                return Err(Error::new(
                    ErrorKind::LexError,
                    "unterminated string",
                    Some(start_span),
                ));
            };

            match byte {
                b'"' => return bytes_to_string(bytes, start_span),
                b'\\' => {
                    bytes.push(b'\\');
                    let Some(escaped) = self.bump_byte()? else {
                        return Err(Error::new(
                            ErrorKind::LexError,
                            "unterminated escape sequence",
                            Some(start_span),
                        ));
                    };
                    bytes.push(escaped);
                    if escaped == b'u' {
                        for _ in 0..4 {
                            let Some(hex) = self.bump_byte()? else {
                                return Err(Error::new(
                                    ErrorKind::LexError,
                                    "incomplete unicode escape",
                                    Some(start_span),
                                ));
                            };
                            bytes.push(hex);
                        }
                    }
                }
                0x00..=0x1F => {
                    return Err(Error::new(
                        ErrorKind::LexError,
                        "invalid control character in string",
                        Some(start_span),
                    ))
                }
                other => bytes.push(other),
            }
        }
    }

    fn skip_string_body(&mut self, _context: &str) -> Result<()> {
        let start_span = self.current_span();
        loop {
            let Some(byte) = self.bump_byte()? else {
                return Err(Error::new(
                    ErrorKind::LexError,
                    "unterminated string",
                    Some(start_span),
                ));
            };
            match byte {
                b'"' => return Ok(()),
                b'\\' => {
                    let Some(escaped) = self.bump_byte()? else {
                        return Err(Error::new(
                            ErrorKind::LexError,
                            "unterminated escape sequence",
                            Some(start_span),
                        ));
                    };
                    if escaped == b'u' {
                        for _ in 0..4 {
                            if self.bump_byte()?.is_none() {
                                return Err(Error::new(
                                    ErrorKind::LexError,
                                    "incomplete unicode escape",
                                    Some(start_span),
                                ));
                            }
                        }
                    }
                }
                0x00..=0x1F => {
                    return Err(Error::new(
                        ErrorKind::LexError,
                        "invalid control character in string",
                        Some(start_span),
                    ))
                }
                _ => {}
            }
        }
    }

    fn skip_insignificant(&mut self) -> Result<()> {
        while let Some(byte) = self.peek_byte()? {
            if byte.is_ascii_whitespace() || byte == 0xEF || byte == 0xBB || byte == 0xBF {
                self.bump_byte()?;
                continue;
            }
            break;
        }
        Ok(())
    }

    fn expect_byte(&mut self, expected: u8, context: &str, msg: &str) -> Result<()> {
        match self.bump_byte()? {
            Some(actual) if actual == expected => Ok(()),
            Some(_) => Err(Error::new(
                ErrorKind::ParseError,
                format!("{}: {}", context, msg),
                Some(self.current_span()),
            )),
            None => Err(Error::new(
                ErrorKind::ParseError,
                format!("{}: unexpected end of input", context),
                Some(self.current_span()),
            )),
        }
    }

    fn peek_byte(&mut self) -> Result<Option<u8>> {
        let buf = self
            .source
            .fill_buf()
            .map_err(|err| Error::new(ErrorKind::ParseError, err.to_string(), None))?;
        if buf.is_empty() {
            Ok(None)
        } else {
            Ok(Some(buf[0]))
        }
    }

    fn bump_byte(&mut self) -> Result<Option<u8>> {
        let byte = {
            let buf = self
                .source
                .fill_buf()
                .map_err(|err| Error::new(ErrorKind::ParseError, err.to_string(), None))?;
            if buf.is_empty() {
                None
            } else {
                Some(buf[0])
            }
        };

        if let Some(byte) = byte {
            self.source.consume(1);
            self.offset += 1;
            if byte == b'\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }

        Ok(byte)
    }

    fn current_span(&self) -> Span {
        Span {
            offset: self.offset,
            line: self.line,
            column: self.column,
        }
    }
}

fn bytes_to_string(bytes: Vec<u8>, span: Span) -> Result<String> {
    String::from_utf8(bytes)
        .map_err(|_| Error::new(ErrorKind::LexError, "invalid utf-8", Some(span)))
}

fn parse_tjson_scalar_raw(raw: &str, span: Span) -> Result<ParsedTypedValue> {
    if raw.starts_with('"') {
        let inner = raw
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .ok_or_else(|| {
                Error::new(ErrorKind::ParseError, "invalid quoted string", Some(span))
            })?;
        return crate::typed_parse::unescape_tjson_string(inner)
            .map(ParsedTypedValue::String)
            .map_err(|err| Error::new(ErrorKind::LexError, err.message, Some(span)));
    }

    crate::typed_parse::parse_unit_typed_value(raw, ParseMode::Strict).map_err(|err| {
        if err.span.is_some() {
            err
        } else {
            Error::new(err.kind, err.message, Some(span))
        }
    })
}
