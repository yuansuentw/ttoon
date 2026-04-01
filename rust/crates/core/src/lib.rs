pub mod arrow;
pub mod format_detect;
pub mod ir;
pub mod schema;
pub mod streaming;
pub mod tjson_arrow;
pub mod tjson_parser;
pub mod tjson_serializer;
pub mod token;
pub mod tokenizer;
pub mod ttoon_parser;
pub mod ttoon_serializer;
pub mod typed_fmt;
pub mod typed_parse;
mod typed_value;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod fixture_tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    LexError,
    ParseError,
    /// Arrow bridge 型別守衛錯誤：資料不符合 arrowable 約束，或 RecordBatch 建構失敗。
    ArrowError,
    SerializeError,
    /// Direct transcode 錯誤：包裝解析或序列化階段的底層錯誤。
    TranscodeError,
}

impl ErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LexError => "lex",
            Self::ParseError => "parse",
            Self::ArrowError => "arrow",
            Self::SerializeError => "serialize",
            Self::TranscodeError => "transcode",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscodeOperation {
    TjsonToTtoon,
    TtoonToTjson,
}

impl TranscodeOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TjsonToTtoon => "tjson_to_ttoon",
            Self::TtoonToTjson => "ttoon_to_tjson",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscodePhase {
    Parse,
    Serialize,
}

impl TranscodePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Serialize => "serialize",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscodeError {
    pub operation: TranscodeOperation,
    pub phase: TranscodePhase,
    pub source_kind: ErrorKind,
    pub source: Box<Error>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
    pub span: Option<Span>,
    pub transcode: Option<TranscodeError>,
}

impl Error {
    pub fn new(kind: ErrorKind, message: impl Into<String>, span: Option<Span>) -> Self {
        Self {
            kind,
            message: message.into(),
            span,
            transcode: None,
        }
    }

    pub fn transcode(operation: TranscodeOperation, phase: TranscodePhase, source: Error) -> Self {
        let message = format!(
            "{}: {} phase failed: {}",
            operation.as_str(),
            phase.as_str(),
            source.message
        );
        let span = source.span;
        let source_kind = source.kind;
        Self {
            kind: ErrorKind::TranscodeError,
            message,
            span,
            transcode: Some(TranscodeError {
                operation,
                phase,
                source_kind,
                source: Box::new(source),
            }),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub use schema::{FieldType, ScalarType, StreamField, StreamSchema};
pub use streaming::{
    ArrowStreamReader, ArrowStreamWriter, StreamReader, StreamResult, StreamWriter,
    TjsonArrowStreamReader, TjsonArrowStreamWriter, TjsonStreamReader, TjsonStreamWriter,
};
pub use typed_parse::ParseMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryFormat {
    Hex,
    B64,
}

impl BinaryFormat {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "hex" => Some(Self::Hex),
            "b64" => Some(Self::B64),
            _ => None,
        }
    }
}

impl Default for BinaryFormat {
    fn default() -> Self {
        Self::Hex
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiter {
    Comma,
    Tab,
    Pipe,
}

impl Delimiter {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "," => Some(Self::Comma),
            "\t" => Some(Self::Tab),
            "|" => Some(Self::Pipe),
            _ => None,
        }
    }
}

impl Default for Delimiter {
    fn default() -> Self {
        Self::Comma
    }
}

// ─── Public API（R11–R16）─────────────────────────────────────────────────────

/// 反序列化任意 typed 文字為 Node。
/// 自動偵測格式（T-JSON / T-TOON），支援任意頂層值（物件、列表、純量）。
/// 此 API 屬 public IR facade，回傳 `Node` 是既有契約，不代表內部值層
/// 應優先 materialize 成 `Node`。
pub fn from_ttoon(text: &str) -> Result<ir::Node> {
    from_ttoon_with_mode(text, ParseMode::Compat)
}

/// 反序列化任意 typed 文字為 Node，並顯式指定 T-TOON / TypedUnit 路徑的 ParseMode。
/// T-JSON 路徑一律維持 Strict，不受此參數影響。
/// 此 API 保留為 public `Node` facade；crate 內新實作應優先走 typed-value 內核。
pub fn from_ttoon_with_mode(text: &str, mode: ParseMode) -> Result<ir::Node> {
    tjson_parser::parse_value_with_mode(text, mode)
}

/// 將 Node 序列化為 T-TOON 文字。
/// 自動選擇輸出形式（Object → 縮排結構；tabular List → `[N]{fields}:` 表格；其他 → `- item`）。
pub fn to_ttoon(node: &ir::Node, opts: Option<&TtoonOptions>) -> Result<String> {
    let default_opts;
    let opts = match opts {
        Some(o) => o,
        None => {
            default_opts = TtoonOptions::default();
            &default_opts
        }
    };
    ttoon_serializer::serialize_to_ttoon_structure(node, opts)
}

/// 將 ArrowTable 序列化為 T-TOON Tabular 格式文字（R14）。
pub fn arrow_to_ttoon(table: &ir::ArrowTable, opts: Option<&TtoonOptions>) -> Result<String> {
    let default_opts;
    let opts = match opts {
        Some(o) => o,
        None => {
            default_opts = TtoonOptions::default();
            &default_opts
        }
    };
    arrow::serialize_arrow_table_to_ttoon(table, opts)
}

/// 將 Node 序列化為 T-JSON 文字（R16）。
pub fn to_tjson(node: &ir::Node, opts: Option<&TjsonOptions>) -> Result<String> {
    let default_opts;
    let opts = match opts {
        Some(o) => o,
        None => {
            default_opts = TjsonOptions::default();
            &default_opts
        }
    };
    tjson_serializer::serialize_tjson(node, opts)
}

/// 將 ArrowTable 序列化為 T-JSON 文字（R16）。
/// 輸出為 `[{col: val, ...}, ...]` 格式（List of Objects）。
pub fn arrow_to_tjson(table: &ir::ArrowTable, opts: Option<&TjsonOptions>) -> Result<String> {
    let default_opts;
    let opts = match opts {
        Some(o) => o,
        None => {
            default_opts = TjsonOptions::default();
            &default_opts
        }
    };
    arrow::serialize_arrow_table_to_tjson(table, opts)
}

pub fn detect_format(input: &str) -> format_detect::Format {
    format_detect::detect(input)
}

// ─── Direct Transcode API ───────────────────────────────────────────────────

/// T-JSON → T-TOON direct transcode。
/// 走專用 T-JSON parser（不經 auto-detect），再序列化為 T-TOON。
/// T-JSON 路徑固定 strict，不接受 ParseMode。
/// 此 API 仍屬 public facade，因此中間會 materialize `Node`。
pub fn tjson_to_ttoon(text: &str, opts: Option<&TtoonOptions>) -> Result<String> {
    let node = tjson_parser::parse_structure(text).map_err(|e| {
        Error::transcode(TranscodeOperation::TjsonToTtoon, TranscodePhase::Parse, e)
    })?;
    to_ttoon(&node, opts).map_err(|e| {
        Error::transcode(
            TranscodeOperation::TjsonToTtoon,
            TranscodePhase::Serialize,
            e,
        )
    })
}

/// T-TOON → T-JSON direct transcode。
/// 走 T-TOON parser（parser-driven mismatch，不做 detect_format 預檢），再序列化為 T-JSON。
/// `mode` 控制 T-TOON / TypedUnit 路徑的解析模式（預設 Compat）。
/// 此 API 仍屬 public facade，因此中間會 materialize `Node`。
pub fn ttoon_to_tjson(text: &str, mode: ParseMode, opts: Option<&TjsonOptions>) -> Result<String> {
    let node = ttoon_parser::parse_ttoon(text, mode).map_err(|e| {
        Error::transcode(TranscodeOperation::TtoonToTjson, TranscodePhase::Parse, e)
    })?;
    to_tjson(&node, opts).map_err(|e| {
        Error::transcode(
            TranscodeOperation::TtoonToTjson,
            TranscodePhase::Serialize,
            e,
        )
    })
}

/// 解析文字為 ArrowTable（R13）。
/// 自動偵測格式（T-JSON / T-TOON），要求資料為 2D arrowable 結構，
/// 欄位值必須為純量；nested list/object 一律拒絕。
///
/// - T-JSON batch object path：允許 sparse rows，缺 key 視為 null；
///   schema 欄位順序以整個 batch 的 first-seen order 推導。
/// - T-TOON tabular：以 header 欄位順序與欄位數為準。
/// - T-TOON structure：不提供 sparse schema 推導；缺欄仍視為錯誤。
pub fn read_arrow(text: &str) -> Result<ir::ArrowTable> {
    let format = format_detect::detect(text);
    if matches!(format, format_detect::Format::Tjson) {
        // T-JSON → direct two-pass path (skip Vec<Token> + Node AST)
        return tjson_arrow::read_arrow_tjson_direct(text);
    }

    // T-TOON tabular / Structure / TypedUnit → compatibility Node-based path.
    // New Arrow-facing work should prefer direct / streaming typed-value paths.
    let field_order = ttoon_parser::extract_tabular_fields(text);
    let node = tjson_parser::parse_value(text)?;
    match node {
        ir::Node::List(rows) => arrow::nodes_to_arrow_table_compat(&rows, field_order.as_deref()),
        _ => Err(Error::new(
            ErrorKind::ArrowError,
            "read_arrow: input must be a list of objects; single objects should use loads() instead",
            None,
        )),
    }
}

/// T-TOON 輸出選項（Structure + Tabular 共用）
#[derive(Debug, Clone)]
pub struct TtoonOptions {
    pub binary_format: BinaryFormat,
    /// Indentation size for T-TOON Structure format. Default: 2
    pub indent_size: u8,
    /// Active delimiter for tabular format.
    pub delimiter: Delimiter,
}

impl Default for TtoonOptions {
    fn default() -> Self {
        Self {
            binary_format: BinaryFormat::Hex,
            indent_size: 2,
            delimiter: Delimiter::Comma,
        }
    }
}

/// T-JSON 輸出選項
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TjsonOptions {
    pub binary_format: BinaryFormat,
}

impl Default for TjsonOptions {
    fn default() -> Self {
        Self {
            binary_format: BinaryFormat::Hex,
        }
    }
}
