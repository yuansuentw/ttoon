//! Arrow Bridge 模組
//!
//! 提供 ArrowTable 與 Arrow/Polars 之間的零複製轉換，
//! 以及 Node（parsed IR）→ ArrowTable 的推斷建構。

use arrow_array::builder::{
    BinaryBuilder, BooleanBuilder, Date32Builder, Decimal128Builder, Decimal256Builder,
    FixedSizeBinaryBuilder, Float64Builder, Int64Builder, StringBuilder, Time64MicrosecondBuilder,
    TimestampMicrosecondBuilder,
};
use arrow_array::{Array, ArrayRef, NullArray, RecordBatch};
use arrow_buffer::i256;
use arrow_schema::{DataType, Field, Schema as ArrowSchema, TimeUnit};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Timelike};
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;

use super::ir::{ArrowTable, Node};
use super::typed_value::ParsedTypedValue;
use super::{typed_fmt, BinaryFormat, Error, ErrorKind, Result, TjsonOptions, TtoonOptions};

const UUID_EXTENSION_NAME_KEY: &str = "ARROW:extension:name";
const UUID_EXTENSION_NAME: &str = "arrow.uuid";

pub fn serialize_arrow_table_to_tjson(table: &ArrowTable, opts: &TjsonOptions) -> Result<String> {
    let schema = &table.schema;
    let num_cols = schema.fields().len();

    let mut buf = String::with_capacity(256);
    buf.push('[');

    let mut first_row = true;
    for batch in &table.batches {
        for row_idx in 0..batch.num_rows() {
            if !first_row {
                buf.push_str(", ");
            }
            first_row = false;
            buf.push('{');
            for col_idx in 0..num_cols {
                if col_idx > 0 {
                    buf.push_str(", ");
                }
                let field = schema.field(col_idx);
                let field_name = field.name();
                buf.push('"');
                typed_fmt::escape_tjson_string(&mut buf, field_name);
                buf.push_str("\": ");
                let col = batch.column(col_idx);
                format_arrow_value_to_tjson_buffer(
                    &mut buf,
                    col.as_ref(),
                    row_idx,
                    field,
                    opts.binary_format,
                )?;
            }
            buf.push('}');
        }
    }

    buf.push(']');
    Ok(buf)
}

pub fn serialize_arrow_table_to_ttoon(table: &ArrowTable, opts: &TtoonOptions) -> Result<String> {
    let schema = &table.schema;
    let num_rows = table.num_rows();
    let num_cols = schema.fields().len();

    let mut buf = String::with_capacity(256);

    let delim = opts.delimiter;
    let delim_sym = super::ttoon_serializer::delim_sym(delim);
    let delim_join = super::ttoon_serializer::delim_join(delim);
    let delim_cell = super::ttoon_serializer::delim_cell(delim);

    let field_names: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();
    write!(
        buf,
        "[{}{}]{{{}}}",
        num_rows,
        delim_sym,
        field_names.join(delim_join)
    )
    .unwrap();
    buf.push(':');
    buf.push('\n');

    for batch in &table.batches {
        for row_idx in 0..batch.num_rows() {
            for col_idx in 0..num_cols {
                if col_idx > 0 {
                    buf.push_str(delim_cell);
                }
                let col = batch.column(col_idx);
                let field = schema.field(col_idx);
                format_arrow_value_to_ttoon_buffer(
                    &mut buf,
                    col.as_ref(),
                    row_idx,
                    field,
                    opts.binary_format,
                )?;
            }
            buf.push('\n');
        }
    }

    Ok(buf)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ColumnType {
    Bool,
    Int64,
    Float64,
    Utf8,
    Binary,
    Uuid,
    Date32,
    Time64Microsecond,
    TimestampMicrosecond { has_tz: bool },
    Decimal { int_digits: u8, scale: i8 },
}

pub(crate) fn uuid_metadata() -> HashMap<String, String> {
    [(
        UUID_EXTENSION_NAME_KEY.to_owned(),
        UUID_EXTENSION_NAME.to_owned(),
    )]
    .into_iter()
    .collect()
}

fn is_uuid_field(field: &Field) -> bool {
    matches!(field.data_type(), DataType::FixedSizeBinary(16))
        && (field.extension_type_name() == Some(UUID_EXTENSION_NAME)
            || field
                .metadata()
                .get(UUID_EXTENSION_NAME_KEY)
                .map(|v| v.as_str())
                == Some(UUID_EXTENSION_NAME))
}

pub(crate) fn data_type_for_column(column_type: &ColumnType) -> Result<DataType> {
    let data_type = match column_type {
        ColumnType::Bool => DataType::Boolean,
        ColumnType::Int64 => DataType::Int64,
        ColumnType::Float64 => DataType::Float64,
        ColumnType::Utf8 => DataType::Utf8,
        ColumnType::Binary => DataType::Binary,
        ColumnType::Uuid => DataType::FixedSizeBinary(16),
        ColumnType::Date32 => DataType::Date32,
        ColumnType::Time64Microsecond => DataType::Time64(TimeUnit::Microsecond),
        ColumnType::TimestampMicrosecond { has_tz } => DataType::Timestamp(
            TimeUnit::Microsecond,
            has_tz.then(|| Arc::<str>::from("UTC")),
        ),
        ColumnType::Decimal { int_digits, scale } => {
            let precision = *int_digits as u16 + *scale as u16;
            if precision == 0 {
                return Err(Error::new(
                    ErrorKind::ArrowError,
                    "read_arrow: decimal precision cannot be zero",
                    None,
                ));
            }
            if precision <= 38 {
                DataType::Decimal128(precision as u8, *scale)
            } else if precision <= 76 {
                DataType::Decimal256(precision as u8, *scale)
            } else {
                return Err(Error::new(
                    ErrorKind::ArrowError,
                    format!(
                        "read_arrow: decimal precision {} exceeds Decimal256",
                        precision
                    ),
                    None,
                ));
            }
        }
    };
    Ok(data_type)
}

pub(crate) fn field_for_column(name: &str, column_type: &ColumnType) -> Result<Field> {
    let data_type = data_type_for_column(column_type)?;
    let field = if matches!(column_type, ColumnType::Uuid) {
        Field::new(name, data_type, true).with_metadata(uuid_metadata())
    } else {
        Field::new(name, data_type, true)
    };
    Ok(field)
}

pub(crate) fn datetime_has_timezone(value: &str) -> bool {
    let Some((_, time_and_tz)) = value.split_once('T') else {
        return false;
    };
    time_and_tz.ends_with('Z') || time_and_tz[8..].contains(['+', '-'])
}

pub(crate) fn decimal_parts(value: &str) -> Result<(u8, i8)> {
    let body = value.strip_suffix('m').ok_or_else(|| {
        Error::new(
            ErrorKind::ArrowError,
            "read_arrow: invalid decimal literal",
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
    Ok((int_part.len() as u8, frac_part.len() as i8))
}

pub(crate) fn column_type_from_node(node: &Node) -> Result<ColumnType> {
    let column_type = match node {
        Node::Bool(_) => ColumnType::Bool,
        Node::Int(_) => ColumnType::Int64,
        Node::Float(_) => ColumnType::Float64,
        Node::String(_) => ColumnType::Utf8,
        Node::Binary(_) => ColumnType::Binary,
        Node::Uuid(_) => ColumnType::Uuid,
        Node::Date(_) => ColumnType::Date32,
        Node::Time(_) => ColumnType::Time64Microsecond,
        Node::DateTime(value) => ColumnType::TimestampMicrosecond {
            has_tz: datetime_has_timezone(value),
        },
        Node::Decimal(value) => {
            let (int_digits, scale) = decimal_parts(value)?;
            ColumnType::Decimal { int_digits, scale }
        }
        Node::Null | Node::List(_) | Node::Object(_) => {
            return Err(Error::new(
                ErrorKind::ArrowError,
                "read_arrow: invalid node for scalar inference",
                None,
            ));
        }
    };
    Ok(column_type)
}

pub(crate) fn column_type_from_typed_value(value: &ParsedTypedValue) -> Result<ColumnType> {
    let column_type = match value {
        ParsedTypedValue::Bool(_) => ColumnType::Bool,
        ParsedTypedValue::Int(_) => ColumnType::Int64,
        ParsedTypedValue::Float(_) => ColumnType::Float64,
        ParsedTypedValue::String(_) => ColumnType::Utf8,
        ParsedTypedValue::Binary(_) => ColumnType::Binary,
        ParsedTypedValue::Uuid(_) => ColumnType::Uuid,
        ParsedTypedValue::Date(_) => ColumnType::Date32,
        ParsedTypedValue::Time(_) => ColumnType::Time64Microsecond,
        ParsedTypedValue::DateTime(value) => ColumnType::TimestampMicrosecond {
            has_tz: datetime_has_timezone(value),
        },
        ParsedTypedValue::Decimal(value) => {
            let (int_digits, scale) = decimal_parts(value)?;
            ColumnType::Decimal { int_digits, scale }
        }
        ParsedTypedValue::Null => {
            return Err(Error::new(
                ErrorKind::ArrowError,
                "read_arrow: invalid typed value for scalar inference",
                None,
            ));
        }
    };
    Ok(column_type)
}

pub(crate) fn merge_column_type(
    existing: &mut ColumnType,
    next: ColumnType,
    field: &str,
) -> Result<()> {
    match (existing, next) {
        (
            ColumnType::Decimal { int_digits, scale },
            ColumnType::Decimal {
                int_digits: next_int,
                scale: next_scale,
            },
        ) => {
            *int_digits = (*int_digits).max(next_int);
            *scale = (*scale).max(next_scale);
            Ok(())
        }
        (
            ColumnType::TimestampMicrosecond { has_tz },
            ColumnType::TimestampMicrosecond {
                has_tz: next_has_tz,
            },
        ) => {
            if *has_tz != next_has_tz {
                return Err(Error::new(
                    ErrorKind::ArrowError,
                    format!(
                        "read_arrow: field '{}' mixes timezone-aware and naive datetime",
                        field
                    ),
                    None,
                ));
            }
            Ok(())
        }
        (current, next) if *current == next => Ok(()),
        (current, next) => Err(Error::new(
            ErrorKind::ArrowError,
            format!(
                "read_arrow: field '{}' has inconsistent types ({:?} vs {:?})",
                field, current, next
            ),
            None,
        )),
    }
}

/// 從 ArrowTable 取得 Arrow Arrays（單 batch 時零複製，多 batch 時 concat）
#[cfg_attr(not(test), allow(dead_code))]
fn table_to_arrow_arrays(table: &ArrowTable) -> Vec<ArrayRef> {
    match table.batches.len() {
        0 => Vec::new(),
        1 => table.batches[0].columns().to_vec(),
        _ => {
            let num_cols = table.schema.fields().len();
            (0..num_cols)
                .map(|col_idx| {
                    let arrays: Vec<&dyn arrow_array::Array> = table
                        .batches
                        .iter()
                        .map(|b| b.column(col_idx).as_ref())
                        .collect();
                    arrow_select::concat::concat(&arrays)
                        .expect("column arrays from same schema should be concat-compatible")
                })
                .collect()
        }
    }
}

/// 要求 rows 為非空的 Object 列表，欄位一致，值均為純量（non-container）。
/// 對 null 值進行 nullable 標記；同列不同型別視為錯誤。
///
/// `field_order`：若提供，則使用此欄位順序建構 schema（保留 tabular header 的原始順序）；
/// 若為 None，則使用第一列 IndexMap 的插入順序。
///
/// 此 helper 僅供既有 `Node`-based compatibility paths 使用；新 Arrow 內核應優先
/// 走 typed-value direct / streaming path，而非先 materialize `Node`。
pub(crate) fn nodes_to_arrow_table_compat(
    rows: &[Node],
    field_order: Option<&[String]>,
) -> Result<ArrowTable> {
    if rows.is_empty() {
        return Err(Error::new(
            ErrorKind::ArrowError,
            "cannot build ArrowTable from empty row list",
            None,
        ));
    }

    // 從 field_order 或第一列取得欄位清單
    let fields: Vec<String> = if let Some(order) = field_order {
        order.to_vec()
    } else {
        match &rows[0] {
            Node::Object(map) => map.keys().cloned().collect(),
            _ => {
                return Err(Error::new(
                    ErrorKind::ArrowError,
                    "read_arrow: all rows must be objects",
                    None,
                ))
            }
        }
    };
    let num_cols = fields.len();

    // 推斷每欄型別（decimal/datetime 會聚合欄位資訊）+ nullable 標記
    let mut col_types: Vec<Option<ColumnType>> = vec![None; num_cols];
    let mut col_nullable: Vec<bool> = vec![false; num_cols];

    for row in rows {
        let obj = match row {
            Node::Object(map) => map,
            _ => {
                return Err(Error::new(
                    ErrorKind::ArrowError,
                    "read_arrow: all rows must be objects",
                    None,
                ))
            }
        };
        if obj.len() != num_cols {
            return Err(Error::new(
                ErrorKind::ArrowError,
                format!(
                    "read_arrow: inconsistent field count ({} vs {})",
                    obj.len(),
                    num_cols
                ),
                None,
            ));
        }
        for (i, field) in fields.iter().enumerate() {
            let value = obj.get(field).ok_or_else(|| {
                Error::new(
                    ErrorKind::ArrowError,
                    format!("read_arrow: missing field '{}'", field),
                    None,
                )
            })?;
            match value {
                Node::Null => {
                    col_nullable[i] = true;
                }
                Node::List(_) | Node::Object(_) => {
                    return Err(Error::new(
                        ErrorKind::ArrowError,
                        format!("read_arrow: field '{}' contains non-scalar value", field),
                        None,
                    ));
                }
                _ => {
                    let next_type = column_type_from_node(value)?;
                    match &mut col_types[i] {
                        None => col_types[i] = Some(next_type),
                        Some(existing) => {
                            merge_column_type(existing, next_type, field)?;
                        }
                    }
                }
            }
        }
    }

    // 建立 Arrow schema。all-null 欄位保留為 Null，不偷偷降級成字串。
    let schema_fields: Vec<Field> = fields
        .iter()
        .enumerate()
        .map(|(i, name)| -> Result<Field> {
            match &col_types[i] {
                Some(column_type) => field_for_column(name, column_type),
                None => Ok(Field::new(name, DataType::Null, true)),
            }
        })
        .collect::<Result<Vec<_>>>()?;
    let schema = Arc::new(ArrowSchema::new(schema_fields.clone()));

    // 建立每欄的 Array
    let num_rows = rows.len();
    let mut arrays: Vec<ArrayRef> = Vec::with_capacity(num_cols);

    for (col_idx, (field_name, schema_field)) in fields.iter().zip(schema_fields.iter()).enumerate()
    {
        let _ = col_idx;
        let col_values: Vec<&Node> = rows
            .iter()
            .map(|row| {
                if let Node::Object(obj) = row {
                    obj.get(field_name).unwrap()
                } else {
                    unreachable!()
                }
            })
            .collect();

        let array: ArrayRef = build_column(col_values, schema_field, num_rows)?;
        arrays.push(array);
    }

    let records = RecordBatch::try_new(schema.clone(), arrays).map_err(|e| {
        Error::new(
            ErrorKind::ArrowError,
            format!("read_arrow: failed to build RecordBatch: {}", e),
            None,
        )
    })?;

    Ok(ArrowTable {
        schema,
        batches: vec![records],
    })
}

/// 將一欄 Node 值轉為 Arrow ArrayRef
fn build_column(values: Vec<&Node>, field: &Field, num_rows: usize) -> Result<ArrayRef> {
    match field.data_type() {
        DataType::Null => Ok(Arc::new(NullArray::new(num_rows))),
        DataType::Boolean => {
            let mut b = BooleanBuilder::new();
            for v in values {
                match v {
                    Node::Null => b.append_null(),
                    Node::Bool(x) => b.append_value(*x),
                    _ => unreachable!(),
                }
            }
            Ok(Arc::new(b.finish()))
        }
        DataType::Int64 => {
            let mut b = Int64Builder::new();
            for v in values {
                match v {
                    Node::Null => b.append_null(),
                    Node::Int(x) => b.append_value(*x),
                    _ => unreachable!(),
                }
            }
            Ok(Arc::new(b.finish()))
        }
        DataType::Float64 => {
            let mut b = Float64Builder::new();
            for v in values {
                match v {
                    Node::Null => b.append_null(),
                    Node::Float(x) => b.append_value(*x),
                    _ => unreachable!(),
                }
            }
            Ok(Arc::new(b.finish()))
        }
        DataType::Date32 => {
            let mut b = Date32Builder::with_capacity(values.len());
            for v in values {
                match v {
                    Node::Null => b.append_null(),
                    Node::Date(value) => b.append_value(date_to_epoch_days(value, "read_arrow")?),
                    _ => unreachable!(),
                }
            }
            Ok(Arc::new(b.finish()))
        }
        DataType::Time64(TimeUnit::Microsecond) => {
            let mut b = Time64MicrosecondBuilder::with_capacity(values.len());
            for v in values {
                match v {
                    Node::Null => b.append_null(),
                    Node::Time(value) => b.append_value(time_to_micros(value, "read_arrow")?),
                    _ => unreachable!(),
                }
            }
            Ok(Arc::new(b.finish()))
        }
        DataType::Timestamp(TimeUnit::Microsecond, _) => {
            let mut b = TimestampMicrosecondBuilder::with_capacity(values.len())
                .with_data_type(field.data_type().clone());
            for v in values {
                match v {
                    Node::Null => b.append_null(),
                    Node::DateTime(value) => {
                        b.append_value(datetime_to_epoch_micros(value, "read_arrow")?)
                    }
                    _ => unreachable!(),
                }
            }
            Ok(Arc::new(b.finish()))
        }
        DataType::Binary => {
            let mut b = BinaryBuilder::new();
            for v in values {
                match v {
                    Node::Null => b.append_null(),
                    Node::Binary(bytes) => b.append_value(bytes),
                    _ => unreachable!(),
                }
            }
            Ok(Arc::new(b.finish()))
        }
        DataType::FixedSizeBinary(byte_width) => {
            let mut b = FixedSizeBinaryBuilder::with_capacity(values.len(), *byte_width);
            for v in values {
                match v {
                    Node::Null => b.append_null(),
                    Node::Uuid(value) if is_uuid_field(field) => {
                        b.append_value(uuid_to_bytes(value, "read_arrow")?.as_slice())
                            .map_err(|e| {
                                Error::new(
                                    ErrorKind::ArrowError,
                                    format!(
                                        "read_arrow: failed to append FixedSizeBinary value: {}",
                                        e
                                    ),
                                    None,
                                )
                            })?;
                    }
                    Node::Binary(bytes) => b.append_value(bytes).map_err(|e| {
                        Error::new(
                            ErrorKind::ArrowError,
                            format!("read_arrow: failed to append FixedSizeBinary value: {}", e),
                            None,
                        )
                    })?,
                    _ => unreachable!(),
                }
            }
            Ok(Arc::new(b.finish()))
        }
        DataType::Utf8 => {
            let mut b = StringBuilder::new();
            for v in values {
                match v {
                    Node::Null => b.append_null(),
                    Node::String(s) => b.append_value(s),
                    _ => unreachable!(),
                }
            }
            Ok(Arc::new(b.finish()))
        }
        DataType::Decimal128(_, scale) => {
            let mut b = Decimal128Builder::with_capacity(values.len())
                .with_data_type(field.data_type().clone());
            for v in values {
                match v {
                    Node::Null => b.append_null(),
                    Node::Decimal(value) => {
                        let scaled = decimal_to_scaled_string(value, *scale, "read_arrow")?;
                        let parsed = scaled.parse::<i128>().map_err(|_| {
                            Error::new(
                                ErrorKind::ArrowError,
                                format!("read_arrow: decimal '{}' exceeds Decimal128", value),
                                None,
                            )
                        })?;
                        b.append_value(parsed);
                    }
                    _ => unreachable!(),
                }
            }
            Ok(Arc::new(b.finish()))
        }
        DataType::Decimal256(_, scale) => {
            let mut b = Decimal256Builder::with_capacity(values.len())
                .with_data_type(field.data_type().clone());
            for v in values {
                match v {
                    Node::Null => b.append_null(),
                    Node::Decimal(value) => {
                        let scaled = decimal_to_scaled_string(value, *scale, "read_arrow")?;
                        let parsed = i256::from_string(&scaled).ok_or_else(|| {
                            Error::new(
                                ErrorKind::ArrowError,
                                format!("read_arrow: decimal '{}' exceeds Decimal256", value),
                                None,
                            )
                        })?;
                        b.append_value(parsed);
                    }
                    _ => unreachable!(),
                }
            }
            Ok(Arc::new(b.finish()))
        }
        _ => Err(Error::new(
            ErrorKind::ArrowError,
            format!("read_arrow: unsupported Arrow type {:?}", field.data_type()),
            None,
        )),
    }
}

pub(crate) fn date_to_epoch_days(value: &str, context: &str) -> Result<i32> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        Error::new(
            ErrorKind::ArrowError,
            format!("{}: invalid date literal '{}'", context, value),
            None,
        )
    })?;
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
    Ok((date - epoch).num_days() as i32)
}

pub(crate) fn time_to_micros(value: &str, context: &str) -> Result<i64> {
    let (main, frac) = match value.split_once('.') {
        Some((main, frac)) => (main, frac),
        None => (value, ""),
    };
    let time = chrono::NaiveTime::parse_from_str(main, "%H:%M:%S").map_err(|_| {
        Error::new(
            ErrorKind::ArrowError,
            format!("{}: invalid time literal '{}'", context, value),
            None,
        )
    })?;
    let frac_padded = format!("{:0<6}", frac);
    let micros = if frac.is_empty() {
        0
    } else {
        frac_padded[..6].parse::<i64>().map_err(|_| {
            Error::new(
                ErrorKind::ArrowError,
                format!("{}: invalid time literal '{}'", context, value),
                None,
            )
        })?
    };
    Ok(time.num_seconds_from_midnight() as i64 * 1_000_000 + micros)
}

pub(crate) fn datetime_to_epoch_micros(value: &str, context: &str) -> Result<i64> {
    if datetime_has_timezone(value) {
        let dt = DateTime::parse_from_rfc3339(value).map_err(|_| {
            Error::new(
                ErrorKind::ArrowError,
                format!("{}: invalid datetime literal '{}'", context, value),
                None,
            )
        })?;
        Ok(dt.to_utc().timestamp_micros())
    } else {
        let dt = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f").map_err(|_| {
            Error::new(
                ErrorKind::ArrowError,
                format!("{}: invalid datetime literal '{}'", context, value),
                None,
            )
        })?;
        Ok(dt.and_utc().timestamp_micros())
    }
}

pub(crate) fn decimal_to_scaled_string(
    value: &str,
    target_scale: i8,
    context: &str,
) -> Result<String> {
    let body = value.strip_suffix('m').ok_or_else(|| {
        Error::new(
            ErrorKind::ArrowError,
            format!("{}: invalid decimal literal '{}'", context, value),
            None,
        )
    })?;
    let (negative, digits) = match body.as_bytes()[0] {
        b'-' => (true, &body[1..]),
        b'+' => (false, &body[1..]),
        _ => (false, body),
    };
    let mut parts = digits.split('.');
    let int_part = parts.next().unwrap_or("");
    let frac_part = parts.next().unwrap_or("");
    let current_scale = frac_part.len() as i8;
    if current_scale > target_scale {
        return Err(Error::new(
            ErrorKind::ArrowError,
            format!(
                "{}: decimal '{}' scale {} exceeds target scale {}",
                context, value, current_scale, target_scale
            ),
            None,
        ));
    }
    let mut scaled = String::new();
    if negative {
        scaled.push('-');
    }
    scaled.push_str(int_part);
    scaled.push_str(frac_part);
    for _ in 0..(target_scale - current_scale) {
        scaled.push('0');
    }
    Ok(scaled)
}

pub(crate) fn uuid_to_bytes(value: &str, context: &str) -> Result<[u8; 16]> {
    let hex = value.replace('-', "");
    if hex.len() != 32 {
        return Err(Error::new(
            ErrorKind::ArrowError,
            format!("{}: invalid uuid literal '{}'", context, value),
            None,
        ));
    }
    let mut bytes = [0u8; 16];
    for (idx, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let part = std::str::from_utf8(chunk).unwrap();
        bytes[idx] = u8::from_str_radix(part, 16).map_err(|_| {
            Error::new(
                ErrorKind::ArrowError,
                format!("{}: invalid uuid literal '{}'", context, value),
                None,
            )
        })?;
    }
    Ok(bytes)
}

fn bytes_to_uuid_string(bytes: &[u8]) -> Result<String> {
    if bytes.len() != 16 {
        return Err(Error::new(
            ErrorKind::SerializeError,
            format!("invalid uuid byte length {}", bytes.len()),
            None,
        ));
    }
    Ok(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    ))
}

/// 從 Arrow Arrays 建立 ArrowTable（零複製）
#[cfg_attr(not(test), allow(dead_code))]
fn arrow_arrays_to_table(arrays: Vec<ArrayRef>, schema: ArrowSchema) -> Result<ArrowTable> {
    let schema = Arc::new(schema);

    let records = RecordBatch::try_new(schema.clone(), arrays).map_err(|e| {
        Error::new(
            ErrorKind::ArrowError,
            &format!("failed to create RecordBatch: {}", e),
            None,
        )
    })?;

    Ok(ArrowTable {
        schema,
        batches: vec![records],
    })
}

// ─── Arrow Value Formatting（R19：從 tjson_serializer 遷入）─────────────────

#[derive(Clone, Copy)]
enum ArrowTextFormat {
    Tjson,
    Ttoon,
}

fn fmt_arrow_string(buffer: &mut String, value: &str, format: ArrowTextFormat) -> Result<()> {
    match format {
        ArrowTextFormat::Tjson => {
            typed_fmt::fmt_tjson_string(buffer, value);
            Ok(())
        }
        ArrowTextFormat::Ttoon => typed_fmt::fmt_ttoon_string(buffer, value),
    }
}

/// Format a value from an Arrow Array at given row index and write directly to buffer.
/// Arrow 模組負責 Array downcast + 值提取；值格式化全部委派 typed_fmt（R19）。
fn format_arrow_value_to_buffer(
    buffer: &mut String,
    array: &dyn Array,
    row_idx: usize,
    field: &Field,
    binary_format: BinaryFormat,
    format: ArrowTextFormat,
) -> Result<()> {
    use arrow_array::*;

    if array.is_null(row_idx) {
        buffer.push_str("null");
        return Ok(());
    }

    let data_type = field.data_type();

    match data_type {
        DataType::Boolean => {
            let arr = array
                .as_any()
                .downcast_ref::<BooleanArray>()
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::SerializeError,
                        "failed to cast to BooleanArray",
                        None,
                    )
                })?;
            typed_fmt::fmt_bool(buffer, arr.value(row_idx));
            Ok(())
        }
        DataType::Int8 => {
            let arr = array.as_any().downcast_ref::<Int8Array>().ok_or_else(|| {
                Error::new(
                    ErrorKind::SerializeError,
                    "failed to cast to Int8Array",
                    None,
                )
            })?;
            typed_fmt::fmt_int(buffer, arr.value(row_idx) as i64);
            Ok(())
        }
        DataType::Int16 => {
            let arr = array.as_any().downcast_ref::<Int16Array>().ok_or_else(|| {
                Error::new(
                    ErrorKind::SerializeError,
                    "failed to cast to Int16Array",
                    None,
                )
            })?;
            typed_fmt::fmt_int(buffer, arr.value(row_idx) as i64);
            Ok(())
        }
        DataType::Int32 => {
            let arr = array.as_any().downcast_ref::<Int32Array>().ok_or_else(|| {
                Error::new(
                    ErrorKind::SerializeError,
                    "failed to cast to Int32Array",
                    None,
                )
            })?;
            typed_fmt::fmt_int(buffer, arr.value(row_idx) as i64);
            Ok(())
        }
        DataType::Int64 => {
            let arr = array.as_any().downcast_ref::<Int64Array>().ok_or_else(|| {
                Error::new(
                    ErrorKind::SerializeError,
                    "failed to cast to Int64Array",
                    None,
                )
            })?;
            typed_fmt::fmt_int(buffer, arr.value(row_idx));
            Ok(())
        }
        DataType::UInt8 | DataType::UInt16 | DataType::UInt32 | DataType::UInt64 => {
            match data_type {
                DataType::UInt8 => {
                    let arr = array.as_any().downcast_ref::<UInt8Array>().ok_or_else(|| {
                        Error::new(
                            ErrorKind::SerializeError,
                            "failed to cast to UInt8Array",
                            None,
                        )
                    })?;
                    typed_fmt::fmt_int(buffer, arr.value(row_idx) as i64);
                }
                DataType::UInt16 => {
                    let arr = array
                        .as_any()
                        .downcast_ref::<UInt16Array>()
                        .ok_or_else(|| {
                            Error::new(
                                ErrorKind::SerializeError,
                                "failed to cast to UInt16Array",
                                None,
                            )
                        })?;
                    typed_fmt::fmt_int(buffer, arr.value(row_idx) as i64);
                }
                DataType::UInt32 => {
                    let arr = array
                        .as_any()
                        .downcast_ref::<UInt32Array>()
                        .ok_or_else(|| {
                            Error::new(
                                ErrorKind::SerializeError,
                                "failed to cast to UInt32Array",
                                None,
                            )
                        })?;
                    typed_fmt::fmt_int(buffer, arr.value(row_idx) as i64);
                }
                DataType::UInt64 => {
                    let arr = array
                        .as_any()
                        .downcast_ref::<UInt64Array>()
                        .ok_or_else(|| {
                            Error::new(
                                ErrorKind::SerializeError,
                                "failed to cast to UInt64Array",
                                None,
                            )
                        })?;
                    let val = arr.value(row_idx);
                    if val > i64::MAX as u64 {
                        return Err(Error::new(
                            ErrorKind::SerializeError,
                            "UInt64 value exceeds i64::MAX",
                            None,
                        ));
                    }
                    typed_fmt::fmt_int(buffer, val as i64);
                }
                _ => unreachable!(),
            }
            Ok(())
        }
        DataType::Float32 => {
            let arr = array
                .as_any()
                .downcast_ref::<Float32Array>()
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::SerializeError,
                        "failed to cast to Float32Array",
                        None,
                    )
                })?;
            typed_fmt::fmt_float(buffer, arr.value(row_idx) as f64);
            Ok(())
        }
        DataType::Float64 => {
            let arr = array
                .as_any()
                .downcast_ref::<Float64Array>()
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::SerializeError,
                        "failed to cast to Float64Array",
                        None,
                    )
                })?;
            typed_fmt::fmt_float(buffer, arr.value(row_idx));
            Ok(())
        }
        DataType::Utf8 => {
            let arr = array
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::SerializeError,
                        "failed to cast to StringArray",
                        None,
                    )
                })?;
            fmt_arrow_string(buffer, arr.value(row_idx), format)
        }
        DataType::LargeUtf8 => {
            let arr = array
                .as_any()
                .downcast_ref::<LargeStringArray>()
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::SerializeError,
                        "failed to cast to LargeStringArray",
                        None,
                    )
                })?;
            fmt_arrow_string(buffer, arr.value(row_idx), format)
        }
        DataType::Binary => {
            let arr = array
                .as_any()
                .downcast_ref::<BinaryArray>()
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::SerializeError,
                        "failed to cast to BinaryArray",
                        None,
                    )
                })?;
            typed_fmt::fmt_binary(buffer, arr.value(row_idx), binary_format)?;
            Ok(())
        }
        DataType::LargeBinary => {
            let arr = array
                .as_any()
                .downcast_ref::<LargeBinaryArray>()
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::SerializeError,
                        "failed to cast to LargeBinaryArray",
                        None,
                    )
                })?;
            typed_fmt::fmt_binary(buffer, arr.value(row_idx), binary_format)?;
            Ok(())
        }
        DataType::FixedSizeBinary(_) => {
            let arr = array
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::SerializeError,
                        "failed to cast to FixedSizeBinaryArray",
                        None,
                    )
                })?;
            let bytes = arr.value(row_idx);
            if is_uuid_field(field) {
                let uuid = bytes_to_uuid_string(bytes)?;
                typed_fmt::fmt_uuid(buffer, &uuid);
            } else {
                typed_fmt::fmt_binary(buffer, bytes, binary_format)?;
            }
            Ok(())
        }
        DataType::Date32 => {
            let arr = array
                .as_any()
                .downcast_ref::<Date32Array>()
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::SerializeError,
                        "failed to cast to Date32Array",
                        None,
                    )
                })?;
            typed_fmt::fmt_date_days(buffer, arr.value(row_idx))?;
            Ok(())
        }
        DataType::Date64 => {
            let arr = array
                .as_any()
                .downcast_ref::<Date64Array>()
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::SerializeError,
                        "failed to cast to Date64Array",
                        None,
                    )
                })?;
            typed_fmt::fmt_date_millis(buffer, arr.value(row_idx))?;
            Ok(())
        }
        DataType::Time32(unit) => {
            let micros = match unit {
                arrow_schema::TimeUnit::Second => {
                    let arr = array
                        .as_any()
                        .downcast_ref::<Time32SecondArray>()
                        .ok_or_else(|| {
                            Error::new(
                                ErrorKind::SerializeError,
                                "failed to cast to Time32SecondArray",
                                None,
                            )
                        })?;
                    arr.value(row_idx) as i64 * 1_000_000
                }
                arrow_schema::TimeUnit::Millisecond => {
                    let arr = array
                        .as_any()
                        .downcast_ref::<Time32MillisecondArray>()
                        .ok_or_else(|| {
                            Error::new(
                                ErrorKind::SerializeError,
                                "failed to cast to Time32MillisecondArray",
                                None,
                            )
                        })?;
                    arr.value(row_idx) as i64 * 1_000
                }
                _ => {
                    return Err(Error::new(
                        ErrorKind::SerializeError,
                        format!("unsupported Time32 unit {:?}", unit),
                        None,
                    ))
                }
            };
            typed_fmt::fmt_time_micros(buffer, micros)?;
            Ok(())
        }
        DataType::Time64(unit) => {
            let micros = match unit {
                arrow_schema::TimeUnit::Microsecond => {
                    let arr = array
                        .as_any()
                        .downcast_ref::<Time64MicrosecondArray>()
                        .ok_or_else(|| {
                            Error::new(
                                ErrorKind::SerializeError,
                                "failed to cast to Time64MicrosecondArray",
                                None,
                            )
                        })?;
                    arr.value(row_idx)
                }
                arrow_schema::TimeUnit::Nanosecond => {
                    let arr = array
                        .as_any()
                        .downcast_ref::<Time64NanosecondArray>()
                        .ok_or_else(|| {
                            Error::new(
                                ErrorKind::SerializeError,
                                "failed to cast to Time64NanosecondArray",
                                None,
                            )
                        })?;
                    arr.value(row_idx) / 1_000
                }
                _ => {
                    return Err(Error::new(
                        ErrorKind::SerializeError,
                        format!("unsupported Time64 unit {:?}", unit),
                        None,
                    ))
                }
            };
            typed_fmt::fmt_time_micros(buffer, micros)?;
            Ok(())
        }
        DataType::Timestamp(unit, tz) => {
            let timestamp_micros = match unit {
                arrow_schema::TimeUnit::Second => {
                    let arr = array
                        .as_any()
                        .downcast_ref::<TimestampSecondArray>()
                        .ok_or_else(|| {
                            Error::new(
                                ErrorKind::SerializeError,
                                "failed to cast to TimestampSecondArray",
                                None,
                            )
                        })?;
                    arr.value(row_idx) * 1_000_000
                }
                arrow_schema::TimeUnit::Millisecond => {
                    let arr = array
                        .as_any()
                        .downcast_ref::<TimestampMillisecondArray>()
                        .ok_or_else(|| {
                            Error::new(
                                ErrorKind::SerializeError,
                                "failed to cast to TimestampMillisecondArray",
                                None,
                            )
                        })?;
                    arr.value(row_idx) * 1_000
                }
                arrow_schema::TimeUnit::Microsecond => {
                    let arr = array
                        .as_any()
                        .downcast_ref::<TimestampMicrosecondArray>()
                        .ok_or_else(|| {
                            Error::new(
                                ErrorKind::SerializeError,
                                "failed to cast to TimestampMicrosecondArray",
                                None,
                            )
                        })?;
                    arr.value(row_idx)
                }
                arrow_schema::TimeUnit::Nanosecond => {
                    let arr = array
                        .as_any()
                        .downcast_ref::<TimestampNanosecondArray>()
                        .ok_or_else(|| {
                            Error::new(
                                ErrorKind::SerializeError,
                                "failed to cast to TimestampNanosecondArray",
                                None,
                            )
                        })?;
                    arr.value(row_idx) / 1_000
                }
            };
            typed_fmt::fmt_timestamp_micros(buffer, timestamp_micros, tz.is_some())?;
            Ok(())
        }
        DataType::Decimal128(_precision, scale) => {
            let arr = array
                .as_any()
                .downcast_ref::<Decimal128Array>()
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::SerializeError,
                        "failed to cast to Decimal128Array",
                        None,
                    )
                })?;
            typed_fmt::fmt_decimal128(buffer, arr.value(row_idx), *scale);
            Ok(())
        }
        DataType::Decimal256(_precision, scale) => {
            let arr = array
                .as_any()
                .downcast_ref::<Decimal256Array>()
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::SerializeError,
                        "failed to cast to Decimal256Array",
                        None,
                    )
                })?;
            let raw = arr.value(row_idx).to_string();
            typed_fmt::fmt_decimal256(buffer, &raw, *scale);
            Ok(())
        }
        _ => Err(Error::new(
            ErrorKind::SerializeError,
            &format!("unsupported arrow type: {:?}", data_type),
            None,
        )),
    }
}

pub(crate) fn format_arrow_value_to_tjson_buffer(
    buffer: &mut String,
    array: &dyn Array,
    row_idx: usize,
    field: &Field,
    binary_format: BinaryFormat,
) -> Result<()> {
    format_arrow_value_to_buffer(
        buffer,
        array,
        row_idx,
        field,
        binary_format,
        ArrowTextFormat::Tjson,
    )
}

pub(crate) fn format_arrow_value_to_ttoon_buffer(
    buffer: &mut String,
    array: &dyn Array,
    row_idx: usize,
    field: &Field,
    binary_format: BinaryFormat,
) -> Result<()> {
    format_arrow_value_to_buffer(
        buffer,
        array,
        row_idx,
        field,
        binary_format,
        ArrowTextFormat::Ttoon,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{Int64Array, StringArray};
    use arrow_schema::{DataType, Field};

    #[test]
    fn test_arrow_arrays_to_table_basic() {
        let schema = ArrowSchema::new(vec![
            Field::new("id", DataType::Int64, false),
            Field::new("name", DataType::Utf8, false),
        ]);

        let id_array = Int64Array::from(vec![1, 2, 3]);
        let name_array = StringArray::from(vec!["Alice", "Bob", "Charlie"]);

        let arrays: Vec<ArrayRef> = vec![Arc::new(id_array), Arc::new(name_array)];

        let result = arrow_arrays_to_table(arrays, schema).unwrap();
        assert_eq!(result.schema.fields().len(), 2);
        assert_eq!(result.num_rows(), 3);
    }

    #[test]
    fn test_table_to_arrow_arrays_basic() {
        let schema = Arc::new(ArrowSchema::new(vec![Field::new(
            "value",
            DataType::Int64,
            false,
        )]));

        let array = Int64Array::from(vec![1, 2, 3]);
        let batch = RecordBatch::try_new(schema.clone(), vec![Arc::new(array)]).unwrap();

        let table = ArrowTable {
            schema,
            batches: vec![batch],
        };

        let arrays = table_to_arrow_arrays(&table);
        assert_eq!(arrays.len(), 1);
        assert_eq!(arrays[0].len(), 3);
    }

    #[test]
    fn test_arrow_arrays_to_table_with_nulls() {
        let schema = ArrowSchema::new(vec![Field::new("value", DataType::Int64, true)]);

        let array = Int64Array::from(vec![Some(1), None, Some(3)]);
        let arrays: Vec<ArrayRef> = vec![Arc::new(array)];

        let result = arrow_arrays_to_table(arrays, schema).unwrap();
        assert_eq!(result.num_rows(), 3);
    }
}
