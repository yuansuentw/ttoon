//! T-JSON → Arrow direct path (skip Node AST).
//!
//! Two-pass design over `&str` input using `TokenIterator`:
//!   Pass 1: infer schema (ColumnType per field) — O(num_cols) memory
//!   Pass 2: build Arrow RecordBatch directly — O(Arrow output) memory
//!
//! Eliminates both Vec<Token> (~980 MB for 1M rows) and Node AST (~500-800 MB),
//! reducing peak memory from ~2.4 GB to ~286 MB for 191 MB T-JSON input.

use std::collections::HashMap;
use std::sync::Arc;

use arrow_array::builder::{
    BinaryBuilder, BooleanBuilder, Date32Builder, Decimal128Builder, Decimal256Builder,
    FixedSizeBinaryBuilder, Float64Builder, Int64Builder, StringBuilder, Time64MicrosecondBuilder,
    TimestampMicrosecondBuilder,
};
use arrow_array::{ArrayRef, NullArray, RecordBatch};
use arrow_buffer::i256;
use arrow_schema::{DataType, Field, Schema as ArrowSchema, TimeUnit};
use indexmap::IndexMap;

use crate::arrow::{
    column_type_from_typed_value, date_to_epoch_days, datetime_to_epoch_micros,
    decimal_to_scaled_string, field_for_column, merge_column_type, time_to_micros, uuid_to_bytes,
    ColumnType,
};
use crate::ir::ArrowTable;
use crate::token::{Token, TokenKind};
use crate::tokenizer::TokenIterator;
use crate::typed_parse;
use crate::typed_value::ParsedTypedValue;
use crate::{Error, ErrorKind, Result};

// ─── Public API ──────────────────────────────────────────────────────────────

/// T-JSON flat objects → ArrowTable (skip-IR direct path).
///
/// Two passes over `&str` using TokenIterator:
///   Pass 1: infer schema (ColumnType per field, O(num_cols) memory)
///   Pass 2: build Arrow columns directly (O(Arrow output) memory)
///
/// No intermediate Vec<Token> or Node AST.
pub(crate) fn read_arrow_tjson_direct(input: &str) -> Result<ArrowTable> {
    let schema = infer_tjson_table_schema(input)?;
    let (arrow_schema, batch) = build_arrow_from_tjson(input, &schema)?;
    Ok(ArrowTable {
        schema: Arc::new(arrow_schema),
        batches: vec![batch],
    })
}

// ─── Schema types ────────────────────────────────────────────────────────────

struct TjsonTableSchema {
    field_names: Vec<String>,
    field_types: Vec<Option<ColumnType>>,
}

// ─── Pass 1: Schema Inference ────────────────────────────────────────────────

fn infer_tjson_table_schema(input: &str) -> Result<TjsonTableSchema> {
    let mut tokens = TokenIterator::new(input);

    expect_token_kind(&mut tokens, "read_arrow", |k| {
        matches!(k, TokenKind::LBracket)
    })?;

    let mut field_names: Vec<String> = Vec::new();
    let mut field_index: HashMap<String, usize> = HashMap::new();
    let mut field_types: Vec<Option<ColumnType>> = Vec::new();
    let mut row_count = 0usize;

    loop {
        let token = next_token(&mut tokens, "read_arrow")?;
        match &token.kind {
            TokenKind::RBracket => break,
            TokenKind::Comma => continue,
            TokenKind::LBrace => {
                let row_entries =
                    parse_object_entries(&mut tokens, "read_arrow", ErrorKind::ArrowError)?;
                infer_object_fields(
                    &row_entries,
                    &mut field_names,
                    &mut field_index,
                    &mut field_types,
                )?;
                row_count += 1;
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::ArrowError,
                    "read_arrow: array items must be objects",
                    Some(token.span),
                ));
            }
        }
    }

    if row_count == 0 {
        return Err(Error::new(
            ErrorKind::ArrowError,
            "cannot build ArrowTable from empty row list",
            None,
        ));
    }

    Ok(TjsonTableSchema {
        field_names,
        field_types,
    })
}

fn infer_object_fields(
    row_entries: &IndexMap<String, Token>,
    field_names: &mut Vec<String>,
    field_index: &mut HashMap<String, usize>,
    field_types: &mut Vec<Option<ColumnType>>,
) -> Result<()> {
    for (key, value_token) in row_entries {
        let col_idx = if let Some(&idx) = field_index.get(key) {
            idx
        } else {
            let idx = field_names.len();
            field_names.push(key.clone());
            field_index.insert(key.clone(), idx);
            field_types.push(None);
            idx
        };

        infer_value_type(value_token, col_idx, field_types, &field_names[col_idx])?;
    }

    Ok(())
}

fn infer_value_type(
    token: &Token,
    col_idx: usize,
    field_types: &mut [Option<ColumnType>],
    field_name: &str,
) -> Result<()> {
    match &token.kind {
        TokenKind::Keyword(kw) if kw == "null" => {
            // null doesn't change inferred type — all fields are nullable in Arrow
        }
        TokenKind::Keyword(kw) if kw == "true" || kw == "false" => {
            merge_inferred(field_types, col_idx, ColumnType::Bool, field_name)?;
        }
        TokenKind::Keyword(kw) if kw == "nan" || kw == "inf" => {
            merge_inferred(field_types, col_idx, ColumnType::Float64, field_name)?;
        }
        TokenKind::Number(raw) => {
            // Use parse_number_like to determine the actual type
            // (handles Date, Time, DateTime, Decimal, Int, Float)
            let token_span = token.span;
            let typed_value = typed_parse::parse_number_like_typed_value(raw, token_span)?;
            let col_type = column_type_from_typed_value(&typed_value)?;
            merge_inferred(field_types, col_idx, col_type, field_name)?;
        }
        TokenKind::String(_) => {
            merge_inferred(field_types, col_idx, ColumnType::Utf8, field_name)?;
        }
        TokenKind::Typed(raw) => {
            // uuid(...), hex(...), b64(...)
            let typed_value =
                typed_parse::parse_unit_typed_value(raw, typed_parse::ParseMode::Strict)
                    .map_err(|e| Error::new(ErrorKind::ArrowError, e.message, Some(token.span)))?;
            let col_type = column_type_from_typed_value(&typed_value)?;
            merge_inferred(field_types, col_idx, col_type, field_name)?;
        }
        TokenKind::LBrace | TokenKind::LBracket => {
            return Err(Error::new(
                ErrorKind::ArrowError,
                format!(
                    "read_arrow: field '{}' contains non-scalar value",
                    field_name
                ),
                Some(token.span),
            ));
        }
        _ => {
            return Err(Error::new(
                ErrorKind::ArrowError,
                format!("read_arrow: unexpected token for field '{}'", field_name),
                Some(token.span),
            ));
        }
    }
    Ok(())
}

fn merge_inferred(
    field_types: &mut [Option<ColumnType>],
    col_idx: usize,
    next_type: ColumnType,
    field_name: &str,
) -> Result<()> {
    match &mut field_types[col_idx] {
        None => field_types[col_idx] = Some(next_type),
        Some(existing) => merge_column_type(existing, next_type, field_name)?,
    }
    Ok(())
}

// ─── Pass 2: Arrow Build ─────────────────────────────────────────────────────

fn build_arrow_from_tjson(
    input: &str,
    schema: &TjsonTableSchema,
) -> Result<(ArrowSchema, RecordBatch)> {
    // Build Arrow schema
    let arrow_fields: Vec<Field> = schema
        .field_names
        .iter()
        .enumerate()
        .map(|(i, name)| match &schema.field_types[i] {
            Some(col_type) => field_for_column(name, col_type),
            None => Ok(Field::new(name, DataType::Null, true)),
        })
        .collect::<Result<Vec<_>>>()?;
    let arrow_schema = ArrowSchema::new(arrow_fields.clone());

    // Create builders
    let mut builders: Vec<ColumnBuilder> = arrow_fields
        .iter()
        .map(|field| ColumnBuilder::new(field.data_type()))
        .collect();

    // Second pass: iterate tokens, parse values directly into builders
    let mut tokens = TokenIterator::new(input);
    expect_token_kind(&mut tokens, "read_arrow", |k| {
        matches!(k, TokenKind::LBracket)
    })?;

    loop {
        let token = next_token(&mut tokens, "read_arrow")?;
        match &token.kind {
            TokenKind::RBracket => break,
            TokenKind::Comma => continue,
            TokenKind::LBrace => {
                let row_entries =
                    parse_object_entries(&mut tokens, "read_arrow", ErrorKind::ArrowError)?;
                build_object_row(
                    &row_entries,
                    &schema.field_names,
                    &arrow_fields,
                    &mut builders,
                )?;
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::ArrowError,
                    "read_arrow: array items must be objects",
                    Some(token.span),
                ));
            }
        }
    }

    // Finish builders → ArrayRef
    let arrays: Vec<ArrayRef> = builders.into_iter().map(|b| b.finish()).collect();

    let batch = RecordBatch::try_new(Arc::new(arrow_schema.clone()), arrays).map_err(|e| {
        Error::new(
            ErrorKind::ArrowError,
            format!("read_arrow: failed to build RecordBatch: {}", e),
            None,
        )
    })?;

    Ok((arrow_schema, batch))
}

fn build_object_row(
    row_entries: &IndexMap<String, Token>,
    field_names: &[String],
    arrow_fields: &[Field],
    builders: &mut [ColumnBuilder],
) -> Result<()> {
    debug_assert_eq!(field_names.len(), arrow_fields.len());
    debug_assert_eq!(field_names.len(), builders.len());

    for (col_idx, field_name) in field_names.iter().enumerate() {
        if let Some(value_token) = row_entries.get(field_name) {
            append_value_to_builder(value_token, &mut builders[col_idx], &arrow_fields[col_idx])?;
        } else {
            builders[col_idx].append_null();
        }
    }

    Ok(())
}

fn parse_object_entries(
    tokens: &mut TokenIterator,
    context: &str,
    error_kind: ErrorKind,
) -> Result<IndexMap<String, Token>> {
    let mut entries = IndexMap::new();

    loop {
        let token = next_token(tokens, context)?;
        match &token.kind {
            TokenKind::RBrace => break,
            TokenKind::Comma => continue,
            TokenKind::String(raw_key) => {
                let key = typed_parse::unescape_tjson_string(raw_key)
                    .map_err(|e| Error::new(ErrorKind::LexError, e.message, Some(token.span)))?;

                expect_token_kind(tokens, context, |k| matches!(k, TokenKind::Colon))?;

                let value_token = next_token(tokens, context)?;
                skip_nested_value(tokens, &value_token, context, error_kind)?;
                entries.insert(key, value_token);
            }
            _ => {
                return Err(Error::new(
                    error_kind,
                    format!("{}: object keys must be strings", context),
                    Some(token.span),
                ));
            }
        }
    }

    Ok(entries)
}

fn skip_nested_value(
    tokens: &mut TokenIterator,
    value_token: &Token,
    context: &str,
    _error_kind: ErrorKind,
) -> Result<()> {
    let mut stack: Vec<TokenKind> = match value_token.kind {
        TokenKind::LBrace => vec![TokenKind::RBrace],
        TokenKind::LBracket => vec![TokenKind::RBracket],
        _ => return Ok(()),
    };

    while let Some(expected_close) = stack.pop() {
        loop {
            let token = next_token(tokens, context)?;
            match token.kind {
                TokenKind::LBrace => {
                    stack.push(expected_close);
                    stack.push(TokenKind::RBrace);
                    break;
                }
                TokenKind::LBracket => {
                    stack.push(expected_close);
                    stack.push(TokenKind::RBracket);
                    break;
                }
                kind if kind == expected_close => break,
                _ => continue,
            }
        }
    }

    Ok(())
}

fn append_value_to_builder(
    token: &Token,
    builder: &mut ColumnBuilder,
    field: &Field,
) -> Result<()> {
    match &token.kind {
        TokenKind::Keyword(kw) if kw == "null" => {
            builder.append_null();
        }
        TokenKind::Keyword(kw) if kw == "true" => {
            builder.append_bool(true)?;
        }
        TokenKind::Keyword(kw) if kw == "false" => {
            builder.append_bool(false)?;
        }
        TokenKind::Keyword(kw) if kw == "nan" => {
            builder.append_float(f64::NAN)?;
        }
        TokenKind::Keyword(kw) if kw == "inf" => {
            builder.append_float(f64::INFINITY)?;
        }
        TokenKind::Number(raw) => {
            // Parse number-like value and append to the appropriate builder
            let typed_value = typed_parse::parse_number_like_typed_value(raw, token.span)?;
            append_typed_value_to_builder(typed_value, builder, field)?;
        }
        TokenKind::String(raw) => {
            let unescaped = typed_parse::unescape_tjson_string(raw)
                .map_err(|e| Error::new(ErrorKind::LexError, e.message, Some(token.span)))?;
            builder.append_string(&unescaped)?;
        }
        TokenKind::Typed(raw) => {
            let typed_value =
                typed_parse::parse_unit_typed_value(raw, typed_parse::ParseMode::Strict)
                    .map_err(|e| Error::new(ErrorKind::ArrowError, e.message, Some(token.span)))?;
            append_typed_value_to_builder(typed_value, builder, field)?;
        }
        _ => {
            return Err(Error::new(
                ErrorKind::ArrowError,
                format!(
                    "read_arrow: unexpected value token for field '{}'",
                    field.name()
                ),
                Some(token.span),
            ));
        }
    }
    Ok(())
}

/// Append a parsed typed value to the appropriate column builder.
/// Used for Number/Typed tokens that produce intermediate typed values.
fn append_typed_value_to_builder(
    value: ParsedTypedValue,
    builder: &mut ColumnBuilder,
    field: &Field,
) -> Result<()> {
    match value {
        ParsedTypedValue::Int(v) => builder.append_int(v),
        ParsedTypedValue::Float(v) => builder.append_float(v),
        ParsedTypedValue::Bool(v) => builder.append_bool(v),
        ParsedTypedValue::String(v) => builder.append_string(&v),
        ParsedTypedValue::Date(v) => builder.append_date(&v),
        ParsedTypedValue::Time(v) => builder.append_time(&v),
        ParsedTypedValue::DateTime(v) => builder.append_datetime(&v),
        ParsedTypedValue::Decimal(v) => builder.append_decimal(&v, field),
        ParsedTypedValue::Binary(v) => builder.append_binary(&v),
        ParsedTypedValue::Uuid(v) => builder.append_uuid(&v, field),
        ParsedTypedValue::Null => {
            builder.append_null();
            Ok(())
        }
    }
}

// ─── Column Builder ──────────────────────────────────────────────────────────

/// Type-dispatched Arrow column builder. One per column.
enum ColumnBuilder {
    Null(usize),
    Bool(BooleanBuilder),
    Int64(Int64Builder),
    Float64(Float64Builder),
    Utf8(StringBuilder),
    Binary(BinaryBuilder),
    Uuid(FixedSizeBinaryBuilder),
    Date32(Date32Builder),
    Time64(Time64MicrosecondBuilder),
    Timestamp(TimestampMicrosecondBuilder),
    Decimal128(Decimal128Builder),
    Decimal256(Decimal256Builder),
}

impl ColumnBuilder {
    fn new(data_type: &DataType) -> Self {
        match data_type {
            DataType::Null => Self::Null(0),
            DataType::Boolean => Self::Bool(BooleanBuilder::new()),
            DataType::Int64 => Self::Int64(Int64Builder::new()),
            DataType::Float64 => Self::Float64(Float64Builder::new()),
            DataType::Utf8 => Self::Utf8(StringBuilder::new()),
            DataType::Binary => Self::Binary(BinaryBuilder::new()),
            DataType::FixedSizeBinary(w) => {
                Self::Uuid(FixedSizeBinaryBuilder::with_capacity(0, *w))
            }
            DataType::Date32 => Self::Date32(Date32Builder::new()),
            DataType::Time64(TimeUnit::Microsecond) => {
                Self::Time64(Time64MicrosecondBuilder::new())
            }
            DataType::Timestamp(TimeUnit::Microsecond, tz) => Self::Timestamp(
                TimestampMicrosecondBuilder::new()
                    .with_data_type(DataType::Timestamp(TimeUnit::Microsecond, tz.clone())),
            ),
            DataType::Decimal128(p, s) => Self::Decimal128(
                Decimal128Builder::new().with_data_type(DataType::Decimal128(*p, *s)),
            ),
            DataType::Decimal256(p, s) => Self::Decimal256(
                Decimal256Builder::new().with_data_type(DataType::Decimal256(*p, *s)),
            ),
            _ => Self::Null(0), // fallback — shouldn't happen
        }
    }

    fn append_null(&mut self) {
        match self {
            Self::Null(n) => *n += 1,
            Self::Bool(b) => b.append_null(),
            Self::Int64(b) => b.append_null(),
            Self::Float64(b) => b.append_null(),
            Self::Utf8(b) => b.append_null(),
            Self::Binary(b) => b.append_null(),
            Self::Uuid(b) => b.append_null(),
            Self::Date32(b) => b.append_null(),
            Self::Time64(b) => b.append_null(),
            Self::Timestamp(b) => b.append_null(),
            Self::Decimal128(b) => b.append_null(),
            Self::Decimal256(b) => b.append_null(),
        }
    }

    fn append_bool(&mut self, v: bool) -> Result<()> {
        match self {
            Self::Bool(b) => {
                b.append_value(v);
                Ok(())
            }
            _ => Err(type_mismatch("bool")),
        }
    }

    fn append_int(&mut self, v: i64) -> Result<()> {
        match self {
            Self::Int64(b) => {
                b.append_value(v);
                Ok(())
            }
            _ => Err(type_mismatch("int")),
        }
    }

    fn append_float(&mut self, v: f64) -> Result<()> {
        match self {
            Self::Float64(b) => {
                b.append_value(v);
                Ok(())
            }
            _ => Err(type_mismatch("float")),
        }
    }

    fn append_string(&mut self, v: &str) -> Result<()> {
        match self {
            Self::Utf8(b) => {
                b.append_value(v);
                Ok(())
            }
            _ => Err(type_mismatch("string")),
        }
    }

    fn append_date(&mut self, v: &str) -> Result<()> {
        match self {
            Self::Date32(b) => {
                b.append_value(date_to_epoch_days(v, "read_arrow")?);
                Ok(())
            }
            _ => Err(type_mismatch("date")),
        }
    }

    fn append_time(&mut self, v: &str) -> Result<()> {
        match self {
            Self::Time64(b) => {
                b.append_value(time_to_micros(v, "read_arrow")?);
                Ok(())
            }
            _ => Err(type_mismatch("time")),
        }
    }

    fn append_datetime(&mut self, v: &str) -> Result<()> {
        match self {
            Self::Timestamp(b) => {
                b.append_value(datetime_to_epoch_micros(v, "read_arrow")?);
                Ok(())
            }
            _ => Err(type_mismatch("datetime")),
        }
    }

    fn append_decimal(&mut self, v: &str, field: &Field) -> Result<()> {
        match (self, field.data_type()) {
            (Self::Decimal128(b), DataType::Decimal128(_, scale)) => {
                let scaled = decimal_to_scaled_string(v, *scale, "read_arrow")?;
                let parsed = scaled.parse::<i128>().map_err(|_| {
                    Error::new(
                        ErrorKind::ArrowError,
                        format!("read_arrow: decimal '{}' exceeds Decimal128", v),
                        None,
                    )
                })?;
                b.append_value(parsed);
                Ok(())
            }
            (Self::Decimal256(b), DataType::Decimal256(_, scale)) => {
                let scaled = decimal_to_scaled_string(v, *scale, "read_arrow")?;
                let parsed = i256::from_string(&scaled).ok_or_else(|| {
                    Error::new(
                        ErrorKind::ArrowError,
                        format!("read_arrow: decimal '{}' exceeds Decimal256", v),
                        None,
                    )
                })?;
                b.append_value(parsed);
                Ok(())
            }
            _ => Err(type_mismatch("decimal")),
        }
    }

    fn append_binary(&mut self, v: &[u8]) -> Result<()> {
        match self {
            Self::Binary(b) => {
                b.append_value(v);
                Ok(())
            }
            _ => Err(type_mismatch("binary")),
        }
    }

    fn append_uuid(&mut self, v: &str, _field: &Field) -> Result<()> {
        match self {
            Self::Uuid(b) => {
                let bytes = uuid_to_bytes(v, "read_arrow")?;
                b.append_value(bytes.as_slice()).map_err(|e| {
                    Error::new(
                        ErrorKind::ArrowError,
                        format!("read_arrow: failed to append UUID: {}", e),
                        None,
                    )
                })?;
                Ok(())
            }
            _ => Err(type_mismatch("uuid")),
        }
    }

    fn finish(self) -> ArrayRef {
        match self {
            Self::Null(n) => Arc::new(NullArray::new(n)),
            Self::Bool(mut b) => Arc::new(b.finish()),
            Self::Int64(mut b) => Arc::new(b.finish()),
            Self::Float64(mut b) => Arc::new(b.finish()),
            Self::Utf8(mut b) => Arc::new(b.finish()),
            Self::Binary(mut b) => Arc::new(b.finish()),
            Self::Uuid(mut b) => Arc::new(b.finish()),
            Self::Date32(mut b) => Arc::new(b.finish()),
            Self::Time64(mut b) => Arc::new(b.finish()),
            Self::Timestamp(mut b) => Arc::new(b.finish()),
            Self::Decimal128(mut b) => Arc::new(b.finish()),
            Self::Decimal256(mut b) => Arc::new(b.finish()),
        }
    }
}

fn type_mismatch(expected: &str) -> Error {
    Error::new(
        ErrorKind::ArrowError,
        format!("read_arrow: builder type mismatch (expected {})", expected),
        None,
    )
}

// ─── Token helpers ───────────────────────────────────────────────────────────

fn next_token(tokens: &mut TokenIterator, context: &str) -> Result<Token> {
    match tokens.next() {
        Some(Ok(token)) => Ok(token),
        Some(Err(e)) => Err(e),
        None => Err(Error::new(
            ErrorKind::ArrowError,
            format!("{}: unexpected end of input", context),
            None,
        )),
    }
}

fn expect_token_kind(
    tokens: &mut TokenIterator,
    context: &str,
    predicate: impl Fn(&TokenKind) -> bool,
) -> Result<Token> {
    let token = next_token(tokens, context)?;
    if predicate(&token.kind) {
        Ok(token)
    } else {
        Err(Error::new(
            ErrorKind::ArrowError,
            format!("{}: unexpected token", context),
            Some(token.span),
        ))
    }
}
