use crate::ir::Node;

/// crate-private scalar semantic kernel.
///
/// `ParsedTypedValue` 只承擔 typed scalar/null 的語義，不表達 `List` / `Object`
/// 這類容器。crate 內新熱路徑應優先在此層完成 parse / validation / Arrow
/// inference，再只於 public IR / compatibility 邊界轉成 `Node`。
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ParsedTypedValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Decimal(String),
    String(String),
    Date(String),
    Time(String),
    DateTime(String),
    Uuid(String),
    Binary(Vec<u8>),
}

/// Compatibility adapter for public `Node`-based IR surfaces.
impl From<ParsedTypedValue> for Node {
    fn from(value: ParsedTypedValue) -> Self {
        match value {
            ParsedTypedValue::Null => Node::Null,
            ParsedTypedValue::Bool(value) => Node::Bool(value),
            ParsedTypedValue::Int(value) => Node::Int(value),
            ParsedTypedValue::Float(value) => Node::Float(value),
            ParsedTypedValue::Decimal(value) => Node::Decimal(value),
            ParsedTypedValue::String(value) => Node::String(value),
            ParsedTypedValue::Date(value) => Node::Date(value),
            ParsedTypedValue::Time(value) => Node::Time(value),
            ParsedTypedValue::DateTime(value) => Node::DateTime(value),
            ParsedTypedValue::Uuid(value) => Node::Uuid(value),
            ParsedTypedValue::Binary(value) => Node::Binary(value),
        }
    }
}
