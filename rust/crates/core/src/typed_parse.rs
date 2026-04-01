//! 值層解析 SSOT（typed unit / keyword / number-like）
//!
//! T-JSON 與 T-TOON 共用同一套值層解析邏輯；型別驗證融入解析，不存在獨立驗證階段。
//! 此模組不依賴 Tokenizer，完全自足。
//!
//! ## 公開入口
//! - [`parse_unit`]：T-TOON/T-JSON typed unit 字串解析
//! - [`parse_keyword`]：純 Keyword token 解析（null/true/false/inf/nan）
//! - [`parse_number_like`]：Number token 解析（含日期/時間/Decimal 驗證）
//! - [`unescape_ttoon_string`]：T-TOON 字串 unescape（僅 5 種 escape）
//! - [`unescape_tjson_string`]：T-JSON 字串 unescape（JSON 完整 escape）

use super::ir::Node;
use super::typed_value::ParsedTypedValue;
use super::{Error, ErrorKind, Result, Span};
use base64::Engine;

// ─── Parse Mode ──────────────────────────────────────────────────────────────

/// 解析模式：控制遇到無法識別的 bare token 時的行為。
///
/// - `Strict`：不認識的 bare token 視為錯誤（適用於 T-TOON serializer 產出的資料）
/// - `Compat`：不認識的 bare token fallback 為字串（相容原始 TOON v3.0 的 bare string）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseMode {
    /// 嚴格模式：未知 bare token → 錯誤。推薦預設值。
    Strict,
    /// 相容模式：未知 bare token → 字串（相容 TOON v3.0 bare string）。
    Compat,
}

impl Default for ParseMode {
    fn default() -> Self {
        Self::Strict
    }
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// 解析任意 typed unit 字串（T-TOON 風格，無 span 資訊）。
///
/// 處理順序：quoted string → keyword → wrapper syntax → datetime → date →
/// time → decimal → float → integer → unquoted string fallback（僅 Compat 模式）。
///
/// `mode` 控制最後的 fallback 行為：
/// - `Strict`：不認識的 bare token 回傳 `LexError`
/// - `Compat`：不認識的 bare token 回傳 `Node::String`
///
/// 備註：此函式保留給 public `Node` surface / compatibility path。
/// crate 內新熱路徑應優先改走 `parse_unit_typed_value()`。
pub fn parse_unit(s: &str, mode: ParseMode) -> Result<Node> {
    parse_unit_typed_value(s, mode).map(Into::into)
}

pub(crate) fn parse_unit_typed_value(s: &str, mode: ParseMode) -> Result<ParsedTypedValue> {
    if s.is_empty() {
        return Ok(ParsedTypedValue::String(String::new()));
    }

    // Quoted string: strip surrounding quotes, unescape T-TOON style
    if s.starts_with('"') {
        let inner = s
            .strip_prefix('"')
            .and_then(|t| t.strip_suffix('"'))
            .ok_or_else(|| Error::new(ErrorKind::ParseError, "invalid quoted string", None))?;
        return unescape_ttoon_string(inner).map(ParsedTypedValue::String);
    }

    // Null, Boolean, special floats
    match s {
        "null" => return Ok(ParsedTypedValue::Null),
        "true" => return Ok(ParsedTypedValue::Bool(true)),
        "false" => return Ok(ParsedTypedValue::Bool(false)),
        "inf" | "+inf" => return Ok(ParsedTypedValue::Float(f64::INFINITY)),
        "-inf" => return Ok(ParsedTypedValue::Float(f64::NEG_INFINITY)),
        "nan" => return Ok(ParsedTypedValue::Float(f64::NAN)),
        _ => {}
    }

    // Wrapper syntax: uuid(...), hex(...), b64(...)
    if let Some(inner) = s.strip_prefix("uuid(").and_then(|t| t.strip_suffix(')')) {
        validate_uuid_content(inner, None)?;
        return Ok(ParsedTypedValue::Uuid(inner.to_string()));
    }
    if let Some(inner) = s.strip_prefix("hex(").and_then(|t| t.strip_suffix(')')) {
        // 空 hex() 代表空二進位，合法
        return Ok(ParsedTypedValue::Binary(decode_hex(inner, None)?));
    }
    if let Some(inner) = s.strip_prefix("b64(").and_then(|t| t.strip_suffix(')')) {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(inner)
            .map_err(|_| Error::new(ErrorKind::ParseError, "invalid base64 in typed cell", None))?;
        return Ok(ParsedTypedValue::Binary(bytes));
    }

    // DateTime: contains 'T' and ':'
    if s.contains('T') && s.contains(':') {
        return parse_datetime(s, None);
    }

    // Date: YYYY-MM-DD
    if is_date_pattern(s) {
        return parse_date(s, None);
    }

    // Time: HH:MM:SS
    if is_time_pattern(s) {
        return parse_time(s, None);
    }

    // Decimal: ends with 'm', body looks like a number
    if s.len() >= 2 && s.ends_with('m') {
        let body = &s[..s.len() - 1];
        let digits = if body.starts_with('+') || body.starts_with('-') {
            &body[1..]
        } else {
            body
        };
        if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return parse_decimal(s, None);
        }
    }

    // Float: contains '.' or 'e'
    if s.contains('.') || s.contains('e') || s.contains('E') {
        let normalized = s.strip_prefix('+').unwrap_or(s);
        if let Ok(f) = normalized.parse::<f64>() {
            return Ok(ParsedTypedValue::Float(f));
        }
    }

    // Integer: optional sign, digits with optional '_'
    {
        let rest = s
            .strip_prefix('+')
            .or_else(|| s.strip_prefix('-'))
            .unwrap_or(s);
        if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit() || c == '_') {
            let normalized: String = s.chars().filter(|&c| c != '_').collect();
            if let Ok(i) = normalized.parse::<i64>() {
                return Ok(ParsedTypedValue::Int(i));
            }
        }
    }

    // Fallback: 根據 mode 決定行為
    match mode {
        ParseMode::Compat => Ok(ParsedTypedValue::String(s.to_string())),
        ParseMode::Strict => Err(Error::new(
            ErrorKind::LexError,
            format!("unknown bare token: {:?}", s),
            None,
        )),
    }
}

/// 解析 Keyword token（純關鍵字：null/true/false/inf/nan）。
///
/// 備註：此函式保留給 public `Node` surface / compatibility path。
/// crate 內新熱路徑應優先改走 `parse_keyword_typed_value()`。
pub fn parse_keyword(keyword: &str, span: Span) -> Result<Node> {
    parse_keyword_typed_value(keyword, span).map(Into::into)
}

pub(crate) fn parse_keyword_typed_value(keyword: &str, span: Span) -> Result<ParsedTypedValue> {
    match keyword {
        "null" => Ok(ParsedTypedValue::Null),
        "true" => Ok(ParsedTypedValue::Bool(true)),
        "false" => Ok(ParsedTypedValue::Bool(false)),
        "inf" | "+inf" => Ok(ParsedTypedValue::Float(f64::INFINITY)),
        "-inf" => Ok(ParsedTypedValue::Float(f64::NEG_INFINITY)),
        "nan" => Ok(ParsedTypedValue::Float(f64::NAN)),
        _ => Err(Error::new(
            ErrorKind::ParseError,
            "unknown keyword",
            Some(span),
        )),
    }
}

/// 解析 Number-like token（含日期/時間/Decimal 驗證）。
///
/// 備註：此函式保留給 public `Node` surface / compatibility path。
/// crate 內新熱路徑應優先改走 `parse_number_like_typed_value()`。
pub fn parse_number_like(value: &str, span: Span) -> Result<Node> {
    parse_number_like_typed_value(value, span).map(Into::into)
}

pub(crate) fn parse_number_like_typed_value(value: &str, span: Span) -> Result<ParsedTypedValue> {
    if is_datetime_like(value) {
        return parse_datetime(value, Some(span));
    }
    if is_time_like(value) {
        return parse_time(value, Some(span));
    }
    if is_date_like(value) {
        return parse_date(value, Some(span));
    }
    if value.ends_with('m') {
        return parse_decimal(value, Some(span));
    }
    if value.contains('.') || value.contains('e') || value.contains('E') {
        return parse_float_literal(value, span);
    }
    let parsed = parse_integer_literal(value, span)?;
    Ok(ParsedTypedValue::Int(parsed))
}

// ─── Unescape ────────────────────────────────────────────────────────────────

/// T-TOON 字串 unescape（TOON v3.0 規範：僅允許 5 種 escape）。
///
/// 輸入為引號內的 raw 內容（不含外層 `"`）。
/// 其他任何 escape sequence 均拋錯（MUST reject）。
pub fn unescape_ttoon_string(raw: &str) -> Result<String> {
    let mut result = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some(other) => {
                    return Err(Error::new(
                        ErrorKind::ParseError,
                        format!("invalid escape sequence in T-TOON string: \\{other}"),
                        None,
                    ))
                }
                None => {
                    return Err(Error::new(
                        ErrorKind::ParseError,
                        "unterminated escape sequence",
                        None,
                    ))
                }
            }
        } else {
            result.push(ch);
        }
    }
    Ok(result)
}

/// T-JSON 字串 unescape（JSON 完整 escape 集合）。
///
/// 輸入為引號內的 raw 內容（不含外層 `"`）。
pub fn unescape_tjson_string(raw: &str) -> Result<String> {
    let literal = format!("\"{}\"", raw);
    serde_json::from_str::<String>(&literal)
        .map_err(|err| Error::new(ErrorKind::LexError, err.to_string(), None))
}

// ─── UUID validation (shared SSOT for parser / serializer entry points) ──────

/// 驗證 UUID 內容：36 字元、8-4-4-4-12 小寫 hex + 連字號格式。
pub(crate) fn validate_uuid_content(s: &str, span: Option<Span>) -> Result<()> {
    let bytes = s.as_bytes();
    if bytes.len() != 36 {
        return Err(Error::new(
            ErrorKind::ParseError,
            "invalid uuid length in typed cell",
            span,
        ));
    }
    if bytes[8] != b'-' || bytes[13] != b'-' || bytes[18] != b'-' || bytes[23] != b'-' {
        return Err(Error::new(
            ErrorKind::ParseError,
            "invalid uuid format: missing hyphens",
            span,
        ));
    }
    if !bytes.iter().enumerate().all(|(idx, ch)| {
        if [8, 13, 18, 23].contains(&idx) {
            *ch == b'-'
        } else {
            ch.is_ascii_digit() || (b'a'..=b'f').contains(ch)
        }
    }) {
        return Err(Error::new(
            ErrorKind::ParseError,
            "invalid uuid: must be lowercase hex",
            span,
        ));
    }
    Ok(())
}

/// 解碼 hex 字串為位元組。
/// 空字串返回空 Vec（代表空二進位）。
pub(crate) fn decode_hex(s: &str, span: Option<Span>) -> Result<Vec<u8>> {
    if s.is_empty() {
        return Ok(Vec::new());
    }
    if s.len() % 2 != 0 {
        return Err(Error::new(
            ErrorKind::ParseError,
            "invalid hex length in typed cell",
            span,
        ));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|_| Error::new(ErrorKind::ParseError, "invalid hex character", span))
        })
        .collect()
}

// ─── Internal: date / time / datetime / decimal ───────────────────────────────

fn parse_date(value: &str, span: Option<Span>) -> Result<ParsedTypedValue> {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return Err(Error::new(ErrorKind::ParseError, "invalid date", span));
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(idx, ch)| idx == 4 || idx == 7 || ch.is_ascii_digit())
    {
        return Err(Error::new(ErrorKind::ParseError, "invalid date", span));
    }
    let year = parse_u32(&value[0..4], span)?;
    let month = parse_u32_range(
        &value[5..7],
        1,
        12,
        "invalid date: month out of range",
        span,
    )?;
    let day = parse_u32_range(&value[8..10], 1, 31, "invalid date: day out of range", span)?;
    let max_day = days_in_month(year, month);
    if day > max_day {
        return Err(Error::new(
            ErrorKind::ParseError,
            format!("invalid date: day {} out of range for month {}", day, month),
            span,
        ));
    }
    Ok(ParsedTypedValue::Date(value.to_string()))
}

fn parse_time(value: &str, span: Option<Span>) -> Result<ParsedTypedValue> {
    let bytes = value.as_bytes();
    if bytes.len() < 8 || bytes[2] != b':' || bytes[5] != b':' {
        return Err(Error::new(ErrorKind::ParseError, "invalid time", span));
    }
    parse_u32_range(&value[0..2], 0, 23, "invalid time: hour out of range", span)?;
    parse_u32_range(
        &value[3..5],
        0,
        59,
        "invalid time: minute out of range",
        span,
    )?;
    parse_u32_range(
        &value[6..8],
        0,
        59,
        "invalid time: second out of range",
        span,
    )?;
    if bytes.len() > 8 {
        if bytes[8] != b'.' {
            return Err(Error::new(
                ErrorKind::ParseError,
                "invalid time: expected '.' for fractional seconds",
                span,
            ));
        }
        let frac = &value[9..];
        if frac.is_empty() {
            return Err(Error::new(
                ErrorKind::ParseError,
                "invalid time: empty fractional seconds",
                span,
            ));
        }
        if frac.len() > 6 {
            return Err(Error::new(
                ErrorKind::ParseError,
                "invalid time: fractional seconds exceed 6 digits",
                span,
            ));
        }
        if !frac.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(Error::new(
                ErrorKind::ParseError,
                "invalid time: fractional seconds must be digits",
                span,
            ));
        }
    }
    Ok(ParsedTypedValue::Time(value.to_string()))
}

fn parse_datetime(value: &str, span: Option<Span>) -> Result<ParsedTypedValue> {
    let Some((date_part, time_and_tz)) = value.split_once('T') else {
        return Err(Error::new(ErrorKind::ParseError, "invalid datetime", span));
    };

    // Validate date part
    let db = date_part.as_bytes();
    if db.len() != 10 || db[4] != b'-' || db[7] != b'-' {
        return Err(Error::new(
            ErrorKind::ParseError,
            "invalid datetime: invalid date part",
            span,
        ));
    }
    if !db
        .iter()
        .enumerate()
        .all(|(idx, ch)| idx == 4 || idx == 7 || ch.is_ascii_digit())
    {
        return Err(Error::new(
            ErrorKind::ParseError,
            "invalid datetime: invalid date part",
            span,
        ));
    }
    let year = parse_u32(&date_part[0..4], span)?;
    let month = parse_u32_range(
        &date_part[5..7],
        1,
        12,
        "invalid datetime: month out of range",
        span,
    )?;
    let day = parse_u32_range(
        &date_part[8..10],
        1,
        31,
        "invalid datetime: day out of range",
        span,
    )?;
    let max_day = days_in_month(year, month);
    if day > max_day {
        return Err(Error::new(
            ErrorKind::ParseError,
            format!(
                "invalid datetime: day {} out of range for month {}",
                day, month
            ),
            span,
        ));
    }

    // Split time body from timezone
    let (time_body, tz_part) = if let Some(pos) = time_and_tz.find(['Z', '+', '-']) {
        time_and_tz.split_at(pos)
    } else {
        (time_and_tz, "")
    };

    // Validate time part
    let tb = time_body.as_bytes();
    if tb.len() < 8 || tb[2] != b':' || tb[5] != b':' {
        return Err(Error::new(
            ErrorKind::ParseError,
            "invalid datetime: invalid time part",
            span,
        ));
    }
    parse_u32_range(
        &time_body[0..2],
        0,
        23,
        "invalid datetime: hour out of range",
        span,
    )?;
    parse_u32_range(
        &time_body[3..5],
        0,
        59,
        "invalid datetime: minute out of range",
        span,
    )?;
    parse_u32_range(
        &time_body[6..8],
        0,
        59,
        "invalid datetime: second out of range",
        span,
    )?;
    if tb.len() > 8 {
        if tb[8] != b'.' {
            return Err(Error::new(
                ErrorKind::ParseError,
                "invalid datetime: expected '.' for fractional seconds",
                span,
            ));
        }
        let frac = &time_body[9..];
        if frac.is_empty() || frac.len() > 6 || !frac.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(Error::new(
                ErrorKind::ParseError,
                "invalid datetime: invalid fractional seconds",
                span,
            ));
        }
    }

    // Validate timezone
    if !tz_part.is_empty() && tz_part != "Z" {
        if tz_part.len() != 6 {
            return Err(Error::new(
                ErrorKind::ParseError,
                "invalid datetime: invalid timezone",
                span,
            ));
        }
        let sign = tz_part.as_bytes()[0];
        if sign != b'+' && sign != b'-' {
            return Err(Error::new(
                ErrorKind::ParseError,
                "invalid datetime: invalid timezone sign",
                span,
            ));
        }
        if tz_part.as_bytes()[3] != b':' {
            return Err(Error::new(
                ErrorKind::ParseError,
                "invalid datetime: invalid timezone format",
                span,
            ));
        }
        parse_u32_range(
            &tz_part[1..3],
            0,
            23,
            "invalid datetime: timezone hour out of range",
            span,
        )?;
        parse_u32_range(
            &tz_part[4..6],
            0,
            59,
            "invalid datetime: timezone minute out of range",
            span,
        )?;
    }

    Ok(ParsedTypedValue::DateTime(value.to_string()))
}

fn parse_decimal(value: &str, span: Option<Span>) -> Result<ParsedTypedValue> {
    let Some(body) = value.strip_suffix('m') else {
        return Err(Error::new(
            ErrorKind::ParseError,
            "decimal must end with 'm'",
            span,
        ));
    };
    let (sign, digits_part) = if let Some(rest) = body.strip_prefix('+') {
        (Some('+'), rest)
    } else if let Some(rest) = body.strip_prefix('-') {
        (Some('-'), rest)
    } else {
        (None, body)
    };
    if digits_part.is_empty() {
        return Err(Error::new(ErrorKind::ParseError, "invalid decimal", span));
    }
    let mut parts = digits_part.split('.');
    let int_part = parts.next().unwrap_or("");
    let frac_part = parts.next();
    if parts.next().is_some() {
        return Err(Error::new(ErrorKind::ParseError, "invalid decimal", span));
    }
    if int_part.is_empty() || !int_part.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(Error::new(ErrorKind::ParseError, "invalid decimal", span));
    }
    if int_part.len() > 1 && int_part.starts_with('0') {
        return Err(Error::new(
            ErrorKind::ParseError,
            "invalid decimal: leading zero",
            span,
        ));
    }
    if let Some(frac) = frac_part {
        if frac.is_empty() || !frac.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(Error::new(ErrorKind::ParseError, "invalid decimal", span));
        }
    }
    // Check for negative zero
    if sign == Some('-') && int_part == "0" {
        let is_zero = match frac_part {
            None => true,
            Some(frac) => frac.chars().all(|ch| ch == '0'),
        };
        if is_zero {
            return Err(Error::new(
                ErrorKind::ParseError,
                "decimal cannot be negative zero",
                span,
            ));
        }
    }
    Ok(ParsedTypedValue::Decimal(value.to_string()))
}

// ─── Internal: integer / float parsers (migrated from parser.rs) ─────────────

fn parse_integer_literal(value: &str, span: Span) -> Result<i64> {
    use std::str::FromStr;

    let mut chars = value.chars();
    let sign = match chars.next() {
        Some('+') => Some('+'),
        Some('-') => Some('-'),
        Some(ch) if ch.is_ascii_digit() => None,
        _ => {
            return Err(Error::new(
                ErrorKind::ParseError,
                "invalid integer",
                Some(span),
            ))
        }
    };

    let digits = if sign.is_some() { &value[1..] } else { value };

    if digits == "0" {
        if sign == Some('-') {
            return Err(Error::new(
                ErrorKind::ParseError,
                "integer cannot be -0",
                Some(span),
            ));
        }
        return Ok(0);
    }

    if digits.starts_with('0') {
        return Err(Error::new(
            ErrorKind::ParseError,
            "integer cannot have leading zero",
            Some(span),
        ));
    }

    if digits.contains('_') {
        let parts: Vec<&str> = digits.split('_').collect();
        if parts.len() < 2 {
            return Err(Error::new(
                ErrorKind::ParseError,
                "invalid integer grouping",
                Some(span),
            ));
        }
        let first = parts[0];
        if first.is_empty() || first.len() > 3 {
            return Err(Error::new(
                ErrorKind::ParseError,
                "invalid integer grouping",
                Some(span),
            ));
        }
        if !first.chars().all(|ch| ch.is_ascii_digit()) || first.starts_with('0') {
            return Err(Error::new(
                ErrorKind::ParseError,
                "invalid integer grouping",
                Some(span),
            ));
        }
        for part in parts.iter().skip(1) {
            if part.len() != 3 || !part.chars().all(|ch| ch.is_ascii_digit()) {
                return Err(Error::new(
                    ErrorKind::ParseError,
                    "invalid integer grouping",
                    Some(span),
                ));
            }
        }
    } else if !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(Error::new(
            ErrorKind::ParseError,
            "invalid integer",
            Some(span),
        ));
    }

    let normalized = digits.replace('_', "");
    let parsed = if sign == Some('-') {
        i64::from_str(&format!("-{}", normalized))
    } else {
        i64::from_str(&normalized)
    }
    .map_err(|_| Error::new(ErrorKind::ParseError, "invalid integer", Some(span)))?;

    Ok(parsed)
}

fn parse_float_literal(value: &str, span: Span) -> Result<ParsedTypedValue> {
    use std::str::FromStr;

    if value.contains('_') || value.contains('E') {
        return Err(Error::new(
            ErrorKind::ParseError,
            "invalid float",
            Some(span),
        ));
    }

    let parts: Vec<&str> = value.split('e').collect();
    if parts.len() > 2 {
        return Err(Error::new(
            ErrorKind::ParseError,
            "invalid float",
            Some(span),
        ));
    }

    let base = parts[0];
    if base.is_empty() {
        return Err(Error::new(
            ErrorKind::ParseError,
            "invalid float",
            Some(span),
        ));
    }

    let (sign, base_digits) = split_sign(base);
    if base_digits.is_empty() {
        return Err(Error::new(
            ErrorKind::ParseError,
            "invalid float",
            Some(span),
        ));
    }

    let base_valid = if base_digits.contains('.') {
        let mut base_parts = base_digits.split('.');
        let int_part = base_parts.next().unwrap_or("");
        let frac_part = base_parts.next().unwrap_or("");
        if base_parts.next().is_some() {
            false
        } else if int_part.is_empty() || frac_part.is_empty() {
            false
        } else {
            is_float_int(int_part) && frac_part.chars().all(|ch| ch.is_ascii_digit())
        }
    } else {
        is_float_int(base_digits)
    };

    if !base_valid {
        return Err(Error::new(
            ErrorKind::ParseError,
            "invalid float",
            Some(span),
        ));
    }

    if parts.len() == 2 {
        let exp = parts[1];
        if exp.is_empty() {
            return Err(Error::new(
                ErrorKind::ParseError,
                "invalid float",
                Some(span),
            ));
        }
        let (_, exp_digits) = split_sign(exp);
        if exp_digits.is_empty() || !exp_digits.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(Error::new(
                ErrorKind::ParseError,
                "invalid float",
                Some(span),
            ));
        }
    }

    let normalized = if sign == Some('+') {
        value.strip_prefix('+').unwrap_or(value)
    } else {
        value
    };

    let parsed = f64::from_str(normalized)
        .map_err(|_| Error::new(ErrorKind::ParseError, "invalid float", Some(span)))?;
    Ok(ParsedTypedValue::Float(parsed))
}

fn split_sign(value: &str) -> (Option<char>, &str) {
    if let Some(rest) = value.strip_prefix('+') {
        (Some('+'), rest)
    } else if let Some(rest) = value.strip_prefix('-') {
        (Some('-'), rest)
    } else {
        (None, value)
    }
}

fn is_float_int(value: &str) -> bool {
    if value == "0" {
        return true;
    }
    if value.is_empty() || value.starts_with('0') {
        return false;
    }
    value.chars().all(|ch| ch.is_ascii_digit())
}

// ─── Internal: pattern helpers ────────────────────────────────────────────────

fn is_date_pattern(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, &c)| i == 4 || i == 7 || c.is_ascii_digit())
}

fn is_time_pattern(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 8 && b[2] == b':' && b[5] == b':'
}

fn is_date_like(value: &str) -> bool {
    is_date_pattern(value)
}

fn is_time_like(value: &str) -> bool {
    is_time_pattern(value)
}

fn is_datetime_like(value: &str) -> bool {
    value.contains('T') && value.contains(':')
}

// ─── Internal: number range helpers ──────────────────────────────────────────

fn parse_u32(s: &str, span: Option<Span>) -> Result<u32> {
    s.parse::<u32>()
        .map_err(|_| Error::new(ErrorKind::ParseError, "invalid number", span))
}

fn parse_u32_range(
    s: &str,
    min: u32,
    max: u32,
    msg: &'static str,
    span: Option<Span>,
) -> Result<u32> {
    if !s.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(Error::new(ErrorKind::ParseError, msg, span));
    }
    let n = s
        .parse::<u32>()
        .map_err(|_| Error::new(ErrorKind::ParseError, msg, span))?;
    if n < min || n > max {
        return Err(Error::new(ErrorKind::ParseError, msg, span));
    }
    Ok(n)
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}
