use std::collections::BTreeSet;
use std::sync::Arc;

use arrow_schema::{DataType, Field, Schema as ArrowSchema, TimeUnit};

use crate::{Error, ErrorKind, Result};

const UUID_EXTENSION_NAME_KEY: &str = "ARROW:extension:name";
const UUID_EXTENSION_NAME: &str = "arrow.uuid";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamSchema {
    fields: Vec<StreamField>,
}

impl StreamSchema {
    pub fn new<I, S>(fields: I) -> Self
    where
        I: IntoIterator<Item = (S, FieldType)>,
        S: Into<String>,
    {
        Self::try_new(fields).expect("invalid stream schema")
    }

    pub fn try_new<I, S>(fields: I) -> Result<Self>
    where
        I: IntoIterator<Item = (S, FieldType)>,
        S: Into<String>,
    {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();

        for (name, field_type) in fields {
            let name = name.into();
            validate_field_name(&name)?;
            validate_field_type(&field_type)?;
            if !seen.insert(name.clone()) {
                return Err(Error::new(
                    ErrorKind::ParseError,
                    format!("duplicate stream schema field '{}'", name),
                    None,
                ));
            }
            out.push(StreamField { name, field_type });
        }

        if out.is_empty() {
            return Err(Error::new(
                ErrorKind::ParseError,
                "stream schema must contain at least one field",
                None,
            ));
        }

        Ok(Self { fields: out })
    }

    pub fn from_arrow_schema(schema: &ArrowSchema) -> Result<Self> {
        let mut fields = Vec::with_capacity(schema.fields().len());
        for field in schema.fields() {
            fields.push((
                field.name().clone(),
                FieldType::from_arrow_field(field.as_ref())?,
            ));
        }
        Self::try_new(fields)
    }

    pub fn to_arrow_schema(&self) -> Result<ArrowSchema> {
        let fields = self
            .fields
            .iter()
            .map(StreamField::to_arrow_field)
            .collect::<Result<Vec<_>>>()?;
        Ok(ArrowSchema::new(fields))
    }

    pub fn len(&self) -> usize {
        self.fields.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn fields(&self) -> &[StreamField] {
        &self.fields
    }

    pub fn field(&self, name: &str) -> Option<&StreamField> {
        self.fields.iter().find(|field| field.name == name)
    }

    pub(crate) fn field_names(&self) -> Vec<String> {
        self.fields.iter().map(|field| field.name.clone()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamField {
    name: String,
    field_type: FieldType,
}

impl StreamField {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn field_type(&self) -> &FieldType {
        &self.field_type
    }

    fn to_arrow_field(&self) -> Result<Field> {
        self.field_type.to_arrow_field(&self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldType {
    scalar_type: ScalarType,
    nullable: bool,
}

impl FieldType {
    pub fn new(scalar_type: ScalarType) -> Self {
        Self {
            scalar_type,
            nullable: false,
        }
    }

    pub fn nullable(scalar_type: ScalarType) -> Self {
        Self {
            scalar_type,
            nullable: true,
        }
    }

    pub fn scalar_type(&self) -> &ScalarType {
        &self.scalar_type
    }

    pub fn is_nullable(&self) -> bool {
        self.nullable
    }

    fn to_arrow_field(&self, name: &str) -> Result<Field> {
        let data_type = self.scalar_type.to_arrow_data_type()?;
        let field = if matches!(self.scalar_type, ScalarType::Uuid) {
            Field::new(name, data_type, self.nullable).with_metadata(uuid_metadata())
        } else {
            Field::new(name, data_type, self.nullable)
        };
        Ok(field)
    }

    fn from_arrow_field(field: &Field) -> Result<Self> {
        let scalar_type = ScalarType::from_arrow_field(field)?;
        Ok(Self {
            scalar_type,
            nullable: field.is_nullable(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalarType {
    String,
    Int,
    Float,
    Bool,
    Decimal { precision: u8, scale: i8 },
    Date,
    Time,
    DateTime { has_tz: bool },
    Uuid,
    Binary,
}

impl ScalarType {
    pub fn decimal(precision: u8, scale: i8) -> Self {
        Self::Decimal { precision, scale }
    }

    pub fn datetime() -> Self {
        Self::DateTime { has_tz: true }
    }

    pub fn datetime_naive() -> Self {
        Self::DateTime { has_tz: false }
    }

    pub(crate) fn to_arrow_data_type(&self) -> Result<DataType> {
        let data_type = match self {
            Self::String => DataType::Utf8,
            Self::Int => DataType::Int64,
            Self::Float => DataType::Float64,
            Self::Bool => DataType::Boolean,
            Self::Decimal { precision, scale } => {
                if *precision == 0 {
                    return Err(Error::new(
                        ErrorKind::ArrowError,
                        "stream schema decimal precision cannot be zero",
                        None,
                    ));
                }
                if *precision <= 38 {
                    DataType::Decimal128(*precision, *scale)
                } else if *precision <= 76 {
                    DataType::Decimal256(*precision, *scale)
                } else {
                    return Err(Error::new(
                        ErrorKind::ArrowError,
                        format!(
                            "stream schema decimal precision {} exceeds Decimal256",
                            precision
                        ),
                        None,
                    ));
                }
            }
            Self::Date => DataType::Date32,
            Self::Time => DataType::Time64(TimeUnit::Microsecond),
            Self::DateTime { has_tz } => DataType::Timestamp(
                TimeUnit::Microsecond,
                has_tz.then(|| Arc::<str>::from("UTC")),
            ),
            Self::Uuid => DataType::FixedSizeBinary(16),
            Self::Binary => DataType::Binary,
        };
        Ok(data_type)
    }

    fn from_arrow_field(field: &Field) -> Result<Self> {
        match field.data_type() {
            DataType::Utf8 => Ok(Self::String),
            DataType::Int64 => Ok(Self::Int),
            DataType::Float64 => Ok(Self::Float),
            DataType::Boolean => Ok(Self::Bool),
            DataType::Decimal128(precision, scale) | DataType::Decimal256(precision, scale) => {
                Ok(Self::Decimal {
                    precision: *precision,
                    scale: *scale,
                })
            }
            DataType::Date32 => Ok(Self::Date),
            DataType::Time64(TimeUnit::Microsecond) => Ok(Self::Time),
            DataType::Time64(unit) => Err(Error::new(
                ErrorKind::ArrowError,
                format!(
                    "stream schema only supports Time64(Microsecond), got Time64({:?})",
                    unit
                ),
                None,
            )),
            DataType::Timestamp(TimeUnit::Microsecond, tz) => {
                let has_tz = match tz.as_ref().map(|value| value.as_ref()) {
                    None => false,
                    Some(value) if is_utc_timezone(value) => true,
                    Some(value) => {
                        return Err(Error::new(
                            ErrorKind::ArrowError,
                            format!(
                            "stream schema only supports UTC timezone-aware timestamps, got '{}'",
                            value
                        ),
                            None,
                        ))
                    }
                };
                Ok(Self::DateTime { has_tz })
            }
            DataType::Timestamp(unit, _) => Err(Error::new(
                ErrorKind::ArrowError,
                format!(
                    "stream schema only supports Timestamp(Microsecond), got Timestamp({:?})",
                    unit
                ),
                None,
            )),
            DataType::FixedSizeBinary(16) if is_uuid_field(field) => Ok(Self::Uuid),
            DataType::Binary => Ok(Self::Binary),
            other => Err(Error::new(
                ErrorKind::ArrowError,
                format!(
                    "unsupported Arrow field type for stream schema: {:?}",
                    other
                ),
                None,
            )),
        }
    }
}

fn validate_field_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::new(
            ErrorKind::ParseError,
            "stream schema field name cannot be empty",
            None,
        ));
    }

    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return Err(Error::new(
            ErrorKind::ParseError,
            format!("invalid stream schema field name '{}'", name),
            None,
        ));
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
        return Err(Error::new(
            ErrorKind::ParseError,
            format!("invalid stream schema field name '{}'", name),
            None,
        ));
    }
    Ok(())
}

fn validate_field_type(field_type: &FieldType) -> Result<()> {
    if let ScalarType::Decimal { precision, scale } = field_type.scalar_type() {
        if *precision == 0 {
            return Err(Error::new(
                ErrorKind::ParseError,
                "stream schema decimal precision cannot be zero",
                None,
            ));
        }
        if *scale < 0 {
            return Err(Error::new(
                ErrorKind::ParseError,
                "stream schema decimal scale must be non-negative",
                None,
            ));
        }
        if *scale as u8 > *precision {
            return Err(Error::new(
                ErrorKind::ParseError,
                format!(
                    "stream schema decimal scale {} exceeds precision {}",
                    scale, precision
                ),
                None,
            ));
        }
    }
    Ok(())
}

fn uuid_metadata() -> std::collections::HashMap<String, String> {
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
                .map(|value| value.as_str())
                == Some(UUID_EXTENSION_NAME))
}

fn is_utc_timezone(value: &str) -> bool {
    matches!(value, "UTC" | "Etc/UTC" | "Z" | "+00:00")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_schema_arrow_roundtrip() {
        let schema = StreamSchema::try_new([
            ("name", FieldType::new(ScalarType::String)),
            ("amount", FieldType::nullable(ScalarType::decimal(10, 2))),
            ("created", FieldType::new(ScalarType::datetime())),
            (
                "created_naive",
                FieldType::new(ScalarType::datetime_naive()),
            ),
            ("id", FieldType::new(ScalarType::Uuid)),
        ])
        .unwrap();

        let arrow_schema = schema.to_arrow_schema().unwrap();
        let roundtrip = StreamSchema::from_arrow_schema(&arrow_schema).unwrap();

        assert_eq!(roundtrip, schema);
        let uuid_field = arrow_schema.field_with_name("id").unwrap();
        assert_eq!(
            uuid_field
                .metadata()
                .get(UUID_EXTENSION_NAME_KEY)
                .map(String::as_str),
            Some(UUID_EXTENSION_NAME)
        );
    }

    #[test]
    fn test_stream_schema_rejects_non_utc_timezone() {
        let arrow_schema = ArrowSchema::new(vec![Field::new(
            "created",
            DataType::Timestamp(TimeUnit::Microsecond, Some(Arc::<str>::from("Asia/Taipei"))),
            false,
        )]);

        let err = StreamSchema::from_arrow_schema(&arrow_schema).unwrap_err();
        assert_eq!(err.kind, ErrorKind::ArrowError);
        assert!(err.message.contains("UTC"));
    }

    #[test]
    fn test_stream_schema_rejects_decimal_precision_exceeding_decimal256() {
        let schema =
            StreamSchema::try_new([("amount", FieldType::new(ScalarType::decimal(77, 2)))])
                .unwrap();

        let err = schema.to_arrow_schema().unwrap_err();
        assert_eq!(err.kind, ErrorKind::ArrowError);
        assert!(err.message.contains("Decimal256"));
    }

    #[test]
    fn test_stream_schema_accepts_canonical_uuid_extension_field() {
        let arrow_schema =
            ArrowSchema::new(vec![Field::new("id", DataType::FixedSizeBinary(16), false)
                .with_metadata(
                    [
                        (
                            UUID_EXTENSION_NAME_KEY.to_owned(),
                            UUID_EXTENSION_NAME.to_owned(),
                        ),
                        ("ARROW:extension:metadata".to_owned(), "".to_owned()),
                    ]
                    .into_iter()
                    .collect(),
                )]);

        let schema = StreamSchema::from_arrow_schema(&arrow_schema).unwrap();

        assert_eq!(
            schema,
            StreamSchema::try_new([("id", FieldType::new(ScalarType::Uuid))]).unwrap()
        );
    }
}
