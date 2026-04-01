use std::sync::Arc;

use arrow_array::builder::{
    BinaryBuilder, BooleanBuilder, Date32Builder, Decimal128Builder, Decimal256Builder,
    FixedSizeBinaryBuilder, Float64Builder, Int64Builder, StringBuilder, Time64MicrosecondBuilder,
    TimestampMicrosecondBuilder,
};
use arrow_array::{ArrayRef, RecordBatch};
use arrow_buffer::i256;
use arrow_schema::{DataType, Field, Schema as ArrowSchema};
use indexmap::IndexMap;

use crate::arrow::{
    date_to_epoch_days, datetime_to_epoch_micros, decimal_to_scaled_string, time_to_micros,
    uuid_to_bytes,
};
use crate::ir::Node;
use crate::schema::{ScalarType, StreamField, StreamSchema};
use crate::typed_value::ParsedTypedValue;
use crate::{Error, ErrorKind, Result};

const UUID_EXTENSION_NAME: &str = "arrow.uuid";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamResult {
    pub rows_emitted: usize,
}

pub(crate) struct ArrowBatchSink {
    schema: StreamSchema,
    arrow_schema: Arc<ArrowSchema>,
    batch_size: usize,
    rows_buffered: usize,
    builders: Vec<ArrowStreamArrayBuilder>,
    context: &'static str,
}

impl ArrowBatchSink {
    pub(crate) fn new(
        schema: StreamSchema,
        batch_size: usize,
        context: &'static str,
    ) -> Result<Self> {
        if batch_size == 0 {
            return Err(Error::new(
                ErrorKind::ArrowError,
                format!("{}: batch_size must be greater than zero", context),
                None,
            ));
        }

        let arrow_schema = Arc::new(schema.to_arrow_schema()?);
        let builders = create_arrow_stream_builders(&schema, batch_size)?;
        Ok(Self {
            schema,
            arrow_schema,
            batch_size,
            rows_buffered: 0,
            builders,
            context,
        })
    }

    pub(crate) fn schema(&self) -> &StreamSchema {
        &self.schema
    }

    pub(crate) fn batch_size(&self) -> usize {
        self.batch_size
    }

    pub(crate) fn rows_buffered(&self) -> usize {
        self.rows_buffered
    }

    pub(crate) fn has_buffered_rows(&self) -> bool {
        self.rows_buffered > 0
    }

    pub(crate) fn append_row(&mut self, values: Vec<ArrowValue>) -> Result<()> {
        for (builder, value) in self.builders.iter_mut().zip(values.into_iter()) {
            builder.append(value, self.context)?;
        }
        self.rows_buffered += 1;
        Ok(())
    }

    pub(crate) fn flush_batch(&mut self) -> Result<RecordBatch> {
        let builders = std::mem::replace(
            &mut self.builders,
            create_arrow_stream_builders(&self.schema, self.batch_size)?,
        );
        let arrays = builders
            .into_iter()
            .map(ArrowStreamArrayBuilder::finish)
            .collect::<Result<Vec<_>>>()?;
        let batch = RecordBatch::try_new(self.arrow_schema.clone(), arrays).map_err(|err| {
            Error::new(
                ErrorKind::ArrowError,
                format!("{}: failed to build RecordBatch: {}", self.context, err),
                None,
            )
        })?;
        self.rows_buffered = 0;
        Ok(batch)
    }
}

pub(crate) fn convert_typed_value_to_arrow_value(
    field: &StreamField,
    value: ParsedTypedValue,
    context: &str,
) -> Result<ArrowValue> {
    validate_typed_value(field, &value, ErrorKind::ArrowError, context)?;

    match value {
        ParsedTypedValue::Null => Ok(ArrowValue::Null),
        ParsedTypedValue::Bool(value) => Ok(ArrowValue::Bool(value)),
        ParsedTypedValue::Int(value) => Ok(ArrowValue::Int(value)),
        ParsedTypedValue::Float(value) => Ok(ArrowValue::Float(value)),
        ParsedTypedValue::String(value) => Ok(ArrowValue::String(value)),
        ParsedTypedValue::Binary(value) => Ok(ArrowValue::Binary(value)),
        ParsedTypedValue::Uuid(value) => Ok(ArrowValue::Uuid(uuid_to_bytes(&value, context)?)),
        ParsedTypedValue::Date(value) => {
            Ok(ArrowValue::Date32(date_to_epoch_days(&value, context)?))
        }
        ParsedTypedValue::Time(value) => Ok(ArrowValue::Time64Microsecond(time_to_micros(
            &value, context,
        )?)),
        ParsedTypedValue::DateTime(value) => Ok(ArrowValue::TimestampMicrosecond(
            datetime_to_epoch_micros(&value, context)?,
        )),
        ParsedTypedValue::Decimal(value) => {
            let ScalarType::Decimal { precision, scale } = field.field_type().scalar_type() else {
                unreachable!("validated decimal field must map to ScalarType::Decimal");
            };

            let scaled = decimal_to_scaled_string(&value, *scale, context)?;
            if *precision <= 38 {
                let parsed = scaled.parse::<i128>().map_err(|_| {
                    Error::new(
                        ErrorKind::ArrowError,
                        format!("{}: decimal '{}' exceeds Decimal128", context, value),
                        None,
                    )
                })?;
                Ok(ArrowValue::Decimal128(parsed))
            } else {
                let parsed = i256::from_string(&scaled).ok_or_else(|| {
                    Error::new(
                        ErrorKind::ArrowError,
                        format!("{}: decimal '{}' exceeds Decimal256", context, value),
                        None,
                    )
                })?;
                Ok(ArrowValue::Decimal256(parsed))
            }
        }
    }
}

pub(crate) fn validate_row_keys(
    row: &IndexMap<String, Node>,
    schema: &StreamSchema,
    kind: ErrorKind,
    context: &str,
) -> Result<()> {
    for field in schema.fields() {
        if !row.contains_key(field.name()) {
            return Err(Error::new(
                kind,
                format!("{}: missing field '{}'", context, field.name()),
                None,
            ));
        }
    }

    if let Some(extra) = row.keys().find(|name| schema.field(name).is_none()) {
        return Err(Error::new(
            kind,
            format!("{}: unexpected field '{}'", context, extra),
            None,
        ));
    }

    Ok(())
}

pub(crate) fn validate_field_value(
    field: &StreamField,
    value: &Node,
    kind: ErrorKind,
    context: &str,
) -> Result<()> {
    validate_scalar_value(field, node_scalar_value_ref(value), kind, context)
}

pub(crate) fn validate_typed_value(
    field: &StreamField,
    value: &ParsedTypedValue,
    kind: ErrorKind,
    context: &str,
) -> Result<()> {
    validate_scalar_value(field, parsed_typed_value_ref(value), kind, context)
}

pub(crate) fn validate_record_batch_schema(
    batch: &RecordBatch,
    expected_fields: &[Arc<arrow_schema::Field>],
    context: &str,
) -> Result<()> {
    let actual_fields = batch.schema_ref().fields();
    if actual_fields.len() != expected_fields.len() {
        return Err(Error::new(
            ErrorKind::SerializeError,
            format!(
                "{}: batch has {} fields, schema expects {}",
                context,
                actual_fields.len(),
                expected_fields.len()
            ),
            None,
        ));
    }

    for (expected, actual) in expected_fields.iter().zip(actual_fields.iter()) {
        if expected.name() != actual.name() {
            return Err(Error::new(
                ErrorKind::SerializeError,
                format!(
                    "{}: batch field '{}' does not match schema field '{}'",
                    context,
                    actual.name(),
                    expected.name()
                ),
                None,
            ));
        }
        if !fields_are_compatible(expected.as_ref(), actual.as_ref()) {
            return Err(Error::new(
                ErrorKind::SerializeError,
                format!(
                    "{}: batch field '{}' has incompatible type",
                    context,
                    actual.name()
                ),
                None,
            ));
        }
    }

    Ok(())
}

fn fields_are_compatible(expected: &Field, actual: &Field) -> bool {
    (expected.data_type() == actual.data_type() && expected.metadata() == actual.metadata())
        || (is_uuid_field(expected) && is_uuid_field(actual))
}

fn is_uuid_field(field: &Field) -> bool {
    matches!(field.data_type(), DataType::FixedSizeBinary(16))
        && field.extension_type_name() == Some(UUID_EXTENSION_NAME)
}

pub(crate) fn datetime_has_timezone(value: &str) -> bool {
    let Some((_, time_and_tz)) = value.split_once('T') else {
        return false;
    };
    if time_and_tz.ends_with('Z') {
        return true;
    }
    match time_and_tz.rfind(['+', '-']) {
        Some(index) => index >= 8,
        None => false,
    }
}

pub(crate) fn io_error(kind: ErrorKind, context: &str, err: std::io::Error) -> Error {
    Error::new(kind, format!("{}: io error: {}", context, err), None)
}

fn create_arrow_stream_builders(
    schema: &StreamSchema,
    batch_size: usize,
) -> Result<Vec<ArrowStreamArrayBuilder>> {
    schema
        .fields()
        .iter()
        .map(|field| ArrowStreamArrayBuilder::new(field, batch_size))
        .collect()
}

fn decimal_precision_scale(value: &str, kind: ErrorKind, context: &str) -> Result<(u8, i8)> {
    let body = value.strip_suffix('m').ok_or_else(|| {
        Error::new(
            kind,
            format!("{}: invalid decimal literal '{}'", context, value),
            None,
        )
    })?;
    let digits = body
        .strip_prefix('+')
        .or_else(|| body.strip_prefix('-'))
        .unwrap_or(body);
    let mut parts = digits.split('.');
    let int_part = parts.next().unwrap_or("");
    let frac_part = parts.next().unwrap_or("");
    if parts.next().is_some() {
        return Err(Error::new(
            kind,
            format!("{}: invalid decimal literal '{}'", context, value),
            None,
        ));
    }

    let mut all_digits = String::with_capacity(int_part.len() + frac_part.len());
    all_digits.push_str(int_part);
    all_digits.push_str(frac_part);
    let trimmed = all_digits.trim_start_matches('0');
    let precision = if trimmed.is_empty() {
        1
    } else {
        trimmed.len() as u8
    };

    Ok((precision, frac_part.len() as i8))
}

fn scalar_type_name(scalar_type: &ScalarType) -> &'static str {
    match scalar_type {
        ScalarType::String => "string",
        ScalarType::Int => "int",
        ScalarType::Float => "float",
        ScalarType::Bool => "bool",
        ScalarType::Decimal { .. } => "decimal",
        ScalarType::Date => "date",
        ScalarType::Time => "time",
        ScalarType::DateTime { has_tz: true } => "timezone-aware datetime",
        ScalarType::DateTime { has_tz: false } => "timezone-naive datetime",
        ScalarType::Uuid => "uuid",
        ScalarType::Binary => "binary",
    }
}

#[derive(Clone, Copy)]
enum ScalarValueRef<'a> {
    Null,
    Bool,
    Int,
    Float,
    Decimal(&'a str),
    String,
    Date,
    Time,
    DateTime(&'a str),
    Uuid,
    Binary,
    List,
    Object,
}

fn validate_scalar_value(
    field: &StreamField,
    value: ScalarValueRef<'_>,
    kind: ErrorKind,
    context: &str,
) -> Result<()> {
    if matches!(value, ScalarValueRef::Null) {
        if field.field_type().is_nullable() {
            return Ok(());
        }
        return Err(Error::new(
            kind,
            format!("{}: field '{}' is not nullable", context, field.name()),
            None,
        ));
    }

    match (field.field_type().scalar_type(), value) {
        (ScalarType::String, ScalarValueRef::String)
        | (ScalarType::Int, ScalarValueRef::Int)
        | (ScalarType::Float, ScalarValueRef::Float)
        | (ScalarType::Bool, ScalarValueRef::Bool)
        | (ScalarType::Date, ScalarValueRef::Date)
        | (ScalarType::Time, ScalarValueRef::Time)
        | (ScalarType::Uuid, ScalarValueRef::Uuid)
        | (ScalarType::Binary, ScalarValueRef::Binary) => Ok(()),
        (ScalarType::DateTime { has_tz }, ScalarValueRef::DateTime(value)) => {
            validate_datetime_value(field.name(), *has_tz, value, kind, context)
        }
        (ScalarType::Decimal { precision, scale }, ScalarValueRef::Decimal(value)) => {
            validate_decimal_value(field.name(), *precision, *scale, value, kind, context)
        }
        (scalar_type, actual) => Err(Error::new(
            kind,
            format!(
                "{}: field '{}' expects {}, got {}",
                context,
                field.name(),
                scalar_type_name(scalar_type),
                scalar_value_name(actual)
            ),
            None,
        )),
    }
}

fn validate_datetime_value(
    field_name: &str,
    has_tz: bool,
    value: &str,
    kind: ErrorKind,
    context: &str,
) -> Result<()> {
    let value_has_tz = datetime_has_timezone(value);
    if has_tz != value_has_tz {
        return Err(Error::new(
            kind,
            format!(
                "{}: field '{}' expects {}, got {}",
                context,
                field_name,
                datetime_type_name(has_tz),
                datetime_type_name(value_has_tz)
            ),
            None,
        ));
    }
    Ok(())
}

fn validate_decimal_value(
    field_name: &str,
    precision: u8,
    scale: i8,
    value: &str,
    kind: ErrorKind,
    context: &str,
) -> Result<()> {
    let (value_precision, value_scale) = decimal_precision_scale(value, kind, context)?;
    if value_precision > precision || value_scale > scale {
        return Err(Error::new(
            kind,
            format!(
                "{}: field '{}' expects decimal({}, {}), got decimal({}, {})",
                context, field_name, precision, scale, value_precision, value_scale
            ),
            None,
        ));
    }
    Ok(())
}

fn datetime_type_name(has_tz: bool) -> &'static str {
    if has_tz {
        "timezone-aware datetime"
    } else {
        "timezone-naive datetime"
    }
}

fn scalar_value_name(value: ScalarValueRef<'_>) -> &'static str {
    match value {
        ScalarValueRef::Null => "null",
        ScalarValueRef::Bool => "bool",
        ScalarValueRef::Int => "int",
        ScalarValueRef::Float => "float",
        ScalarValueRef::Decimal(_) => "decimal",
        ScalarValueRef::String => "string",
        ScalarValueRef::Date => "date",
        ScalarValueRef::Time => "time",
        ScalarValueRef::DateTime(value) if datetime_has_timezone(value) => {
            "timezone-aware datetime"
        }
        ScalarValueRef::DateTime(_) => "timezone-naive datetime",
        ScalarValueRef::Uuid => "uuid",
        ScalarValueRef::Binary => "binary",
        ScalarValueRef::List => "list",
        ScalarValueRef::Object => "object",
    }
}

fn node_scalar_value_ref(value: &Node) -> ScalarValueRef<'_> {
    match value {
        Node::Null => ScalarValueRef::Null,
        Node::Bool(_) => ScalarValueRef::Bool,
        Node::Int(_) => ScalarValueRef::Int,
        Node::Float(_) => ScalarValueRef::Float,
        Node::Decimal(value) => ScalarValueRef::Decimal(value),
        Node::String(_) => ScalarValueRef::String,
        Node::Date(_) => ScalarValueRef::Date,
        Node::Time(_) => ScalarValueRef::Time,
        Node::DateTime(value) => ScalarValueRef::DateTime(value),
        Node::Uuid(_) => ScalarValueRef::Uuid,
        Node::Binary(_) => ScalarValueRef::Binary,
        Node::List(_) => ScalarValueRef::List,
        Node::Object(_) => ScalarValueRef::Object,
    }
}

fn parsed_typed_value_ref(value: &ParsedTypedValue) -> ScalarValueRef<'_> {
    match value {
        ParsedTypedValue::Null => ScalarValueRef::Null,
        ParsedTypedValue::Bool(_) => ScalarValueRef::Bool,
        ParsedTypedValue::Int(_) => ScalarValueRef::Int,
        ParsedTypedValue::Float(_) => ScalarValueRef::Float,
        ParsedTypedValue::Decimal(value) => ScalarValueRef::Decimal(value),
        ParsedTypedValue::String(_) => ScalarValueRef::String,
        ParsedTypedValue::Date(_) => ScalarValueRef::Date,
        ParsedTypedValue::Time(_) => ScalarValueRef::Time,
        ParsedTypedValue::DateTime(value) => ScalarValueRef::DateTime(value),
        ParsedTypedValue::Uuid(_) => ScalarValueRef::Uuid,
        ParsedTypedValue::Binary(_) => ScalarValueRef::Binary,
    }
}

enum ArrowStreamArrayBuilder {
    Bool(BooleanBuilder),
    Int64(Int64Builder),
    Float64(Float64Builder),
    Utf8(StringBuilder),
    Binary(BinaryBuilder),
    Uuid(FixedSizeBinaryBuilder),
    Date32(Date32Builder),
    Time64Microsecond(Time64MicrosecondBuilder),
    TimestampMicrosecond(TimestampMicrosecondBuilder),
    Decimal128(Decimal128Builder),
    Decimal256(Decimal256Builder),
}

impl ArrowStreamArrayBuilder {
    fn new(field: &StreamField, capacity: usize) -> Result<Self> {
        let builder = match field.field_type().scalar_type() {
            ScalarType::Bool => Self::Bool(BooleanBuilder::with_capacity(capacity)),
            ScalarType::Int => Self::Int64(Int64Builder::with_capacity(capacity)),
            ScalarType::Float => Self::Float64(Float64Builder::with_capacity(capacity)),
            ScalarType::String => Self::Utf8(StringBuilder::with_capacity(capacity, capacity * 8)),
            ScalarType::Binary => Self::Binary(BinaryBuilder::new()),
            ScalarType::Uuid => Self::Uuid(FixedSizeBinaryBuilder::with_capacity(capacity, 16)),
            ScalarType::Date => Self::Date32(Date32Builder::with_capacity(capacity)),
            ScalarType::Time => {
                Self::Time64Microsecond(Time64MicrosecondBuilder::with_capacity(capacity))
            }
            ScalarType::DateTime { has_tz } => {
                let data_type = ScalarType::DateTime { has_tz: *has_tz }.to_arrow_data_type()?;
                Self::TimestampMicrosecond(
                    TimestampMicrosecondBuilder::with_capacity(capacity).with_data_type(data_type),
                )
            }
            ScalarType::Decimal { precision, scale } => {
                let data_type = ScalarType::Decimal {
                    precision: *precision,
                    scale: *scale,
                }
                .to_arrow_data_type()?;
                if *precision <= 38 {
                    Self::Decimal128(
                        Decimal128Builder::with_capacity(capacity).with_data_type(data_type),
                    )
                } else {
                    Self::Decimal256(
                        Decimal256Builder::with_capacity(capacity).with_data_type(data_type),
                    )
                }
            }
        };
        Ok(builder)
    }

    fn append(&mut self, value: ArrowValue, context: &str) -> Result<()> {
        match (self, value) {
            (Self::Bool(builder), ArrowValue::Null) => builder.append_null(),
            (Self::Bool(builder), ArrowValue::Bool(value)) => builder.append_value(value),
            (Self::Int64(builder), ArrowValue::Null) => builder.append_null(),
            (Self::Int64(builder), ArrowValue::Int(value)) => builder.append_value(value),
            (Self::Float64(builder), ArrowValue::Null) => builder.append_null(),
            (Self::Float64(builder), ArrowValue::Float(value)) => builder.append_value(value),
            (Self::Utf8(builder), ArrowValue::Null) => builder.append_null(),
            (Self::Utf8(builder), ArrowValue::String(value)) => builder.append_value(&value),
            (Self::Binary(builder), ArrowValue::Null) => builder.append_null(),
            (Self::Binary(builder), ArrowValue::Binary(value)) => builder.append_value(&value),
            (Self::Uuid(builder), ArrowValue::Null) => builder.append_null(),
            (Self::Uuid(builder), ArrowValue::Uuid(value)) => {
                builder.append_value(value.as_slice()).map_err(|err| {
                    Error::new(
                        ErrorKind::ArrowError,
                        format!(
                            "{}: failed to append FixedSizeBinary value: {}",
                            context, err
                        ),
                        None,
                    )
                })?;
            }
            (Self::Date32(builder), ArrowValue::Null) => builder.append_null(),
            (Self::Date32(builder), ArrowValue::Date32(value)) => builder.append_value(value),
            (Self::Time64Microsecond(builder), ArrowValue::Null) => builder.append_null(),
            (Self::Time64Microsecond(builder), ArrowValue::Time64Microsecond(value)) => {
                builder.append_value(value)
            }
            (Self::TimestampMicrosecond(builder), ArrowValue::Null) => builder.append_null(),
            (Self::TimestampMicrosecond(builder), ArrowValue::TimestampMicrosecond(value)) => {
                builder.append_value(value)
            }
            (Self::Decimal128(builder), ArrowValue::Null) => builder.append_null(),
            (Self::Decimal128(builder), ArrowValue::Decimal128(value)) => {
                builder.append_value(value)
            }
            (Self::Decimal256(builder), ArrowValue::Null) => builder.append_null(),
            (Self::Decimal256(builder), ArrowValue::Decimal256(value)) => {
                builder.append_value(value)
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::ArrowError,
                    format!("{}: internal builder type mismatch", context),
                    None,
                ))
            }
        }
        Ok(())
    }

    fn finish(self) -> Result<ArrayRef> {
        let array: ArrayRef = match self {
            Self::Bool(mut builder) => Arc::new(builder.finish()),
            Self::Int64(mut builder) => Arc::new(builder.finish()),
            Self::Float64(mut builder) => Arc::new(builder.finish()),
            Self::Utf8(mut builder) => Arc::new(builder.finish()),
            Self::Binary(mut builder) => Arc::new(builder.finish()),
            Self::Uuid(mut builder) => Arc::new(builder.finish()),
            Self::Date32(mut builder) => Arc::new(builder.finish()),
            Self::Time64Microsecond(mut builder) => Arc::new(builder.finish()),
            Self::TimestampMicrosecond(mut builder) => Arc::new(builder.finish()),
            Self::Decimal128(mut builder) => Arc::new(builder.finish()),
            Self::Decimal256(mut builder) => Arc::new(builder.finish()),
        };
        Ok(array)
    }
}

pub(crate) enum ArrowValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Binary(Vec<u8>),
    Uuid([u8; 16]),
    Date32(i32),
    Time64Microsecond(i64),
    TimestampMicrosecond(i64),
    Decimal128(i128),
    Decimal256(i256),
}
