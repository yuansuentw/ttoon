//! Typed value formatting functions for TTOON serialization.
//!
//! Provides format-independent scalar value formatting used by both
//! T-JSON and T-TOON serializers. All typed wrappers use unified `()`
//! syntax: `uuid()`, `hex()`, `b64()`.

use base64::Engine;
use chrono::SecondsFormat;
use std::fmt::Write;

use super::{BinaryFormat, Error, ErrorKind, Result};

/// Format an integer value to buffer.
pub fn fmt_int(buf: &mut String, v: i64) {
    write!(buf, "{}", v).unwrap();
}

/// Format a floating-point value to buffer.
/// Uses ryu for fast, precise formatting. Always includes `.0` for whole numbers.
pub fn fmt_float(buf: &mut String, v: f64) {
    if v.is_nan() {
        buf.push_str("nan");
        return;
    }
    if v == f64::INFINITY {
        buf.push_str("inf");
        return;
    }
    if v == f64::NEG_INFINITY {
        buf.push_str("-inf");
        return;
    }
    let mut b = ryu::Buffer::new();
    let s = b.format(v);
    buf.push_str(s);
    // Ryu omits .0 for pure integers; PRD requires it to distinguish from Int.
    if !s.contains('.') && !s.contains('e') && !s.contains('E') {
        buf.push_str(".0");
    }
}

/// Format a datetime value to buffer (passthrough, no timezone conversion).
pub fn fmt_datetime(buf: &mut String, v: &str) {
    buf.push_str(v);
}

/// Format a UUID value to buffer using unified `uuid()` syntax.
pub fn fmt_uuid(buf: &mut String, v: &str) {
    buf.push_str("uuid(");
    buf.push_str(v);
    buf.push(')');
}

/// Format a binary value to buffer using unified typed syntax.
///
/// Uses `hex()` or `b64()` wrapper depending on `opts.binary_format`.
pub fn fmt_binary(buf: &mut String, v: &[u8], binary_format: BinaryFormat) -> Result<()> {
    match binary_format {
        BinaryFormat::Hex => {
            buf.push_str("hex(");
            encode_hex(buf, v);
            buf.push(')');
            Ok(())
        }
        BinaryFormat::B64 => {
            let engine = base64::engine::general_purpose::STANDARD;
            buf.push_str("b64(");
            engine.encode_string(v, buf);
            buf.push(')');
            Ok(())
        }
    }
}

/// Escape a T-JSON string value and write to buffer (zero allocation).
pub fn escape_tjson_string(buf: &mut String, v: &str) {
    for ch in v.chars() {
        match ch {
            '"' => buf.push_str("\\\""),
            '\\' => buf.push_str("\\\\"),
            '\n' => buf.push_str("\\n"),
            '\r' => buf.push_str("\\r"),
            '\t' => buf.push_str("\\t"),
            '\u{08}' => buf.push_str("\\b"),
            '\u{0C}' => buf.push_str("\\f"),
            ch if ch.is_control() => {
                use std::fmt::Write;
                write!(buf, "\\u{:04X}", ch as u32).unwrap();
            }
            _ => buf.push(ch),
        }
    }
}

/// Escape a T-TOON string value and write to buffer (zero allocation).
pub fn escape_ttoon_string(buf: &mut String, v: &str) -> Result<()> {
    for ch in v.chars() {
        match ch {
            '"' => buf.push_str("\\\""),
            '\\' => buf.push_str("\\\\"),
            '\n' => buf.push_str("\\n"),
            '\r' => buf.push_str("\\r"),
            '\t' => buf.push_str("\\t"),
            ch if ch.is_control() => {
                return Err(Error::new(
                    ErrorKind::SerializeError,
                    format!(
                        "T-TOON string contains unsupported control character U+{:04X}",
                        ch as u32
                    ),
                    None,
                ));
            }
            _ => buf.push(ch),
        }
    }
    Ok(())
}

/// Format a boolean value to buffer ("true" / "false").
pub fn fmt_bool(buf: &mut String, v: bool) {
    buf.push_str(if v { "true" } else { "false" });
}

/// Format a quoted T-JSON string value to buffer.
pub fn fmt_tjson_string(buf: &mut String, v: &str) {
    buf.push('"');
    escape_tjson_string(buf, v);
    buf.push('"');
}

/// Format a quoted T-TOON string value to buffer.
pub fn fmt_ttoon_string(buf: &mut String, v: &str) -> Result<()> {
    buf.push('"');
    escape_ttoon_string(buf, v)?;
    buf.push('"');
    Ok(())
}

/// Format a Date32 (days since 1970-01-01) as YYYY-MM-DD.
pub fn fmt_date_days(buf: &mut String, days: i32) -> Result<()> {
    let base = chrono::NaiveDate::from_ymd_opt(1970, 1, 1)
        .ok_or_else(|| Error::new(ErrorKind::SerializeError, "invalid base date", None))?;
    let date = base
        .checked_add_signed(chrono::Duration::days(days as i64))
        .ok_or_else(|| Error::new(ErrorKind::SerializeError, "date overflow", None))?;
    write!(buf, "{}", date.format("%Y-%m-%d")).unwrap();
    Ok(())
}

/// Format a Date64 (milliseconds since epoch) as YYYY-MM-DD.
pub fn fmt_date_millis(buf: &mut String, millis: i64) -> Result<()> {
    let date = chrono::DateTime::from_timestamp(millis / 1000, 0)
        .map(|dt| dt.naive_utc())
        .ok_or_else(|| Error::new(ErrorKind::SerializeError, "invalid timestamp", None))?;
    write!(buf, "{}", date.format("%Y-%m-%d")).unwrap();
    Ok(())
}

/// Format a Timestamp (microseconds since epoch) as ISO 8601.
/// If `has_tz` is true, outputs RFC3339 with UTC "Z" suffix; otherwise naive local datetime.
pub fn fmt_timestamp_micros(buf: &mut String, micros: i64, has_tz: bool) -> Result<()> {
    let seconds = micros / 1_000_000;
    let nanos = ((micros % 1_000_000) * 1_000) as u32;
    if has_tz {
        let utc_dt = chrono::DateTime::from_timestamp(seconds, nanos)
            .ok_or_else(|| Error::new(ErrorKind::SerializeError, "invalid timestamp", None))?;
        buf.push_str(&utc_dt.to_rfc3339_opts(SecondsFormat::AutoSi, true));
    } else {
        let naive_dt = chrono::DateTime::from_timestamp(seconds, nanos)
            .map(|dt| dt.naive_utc())
            .ok_or_else(|| Error::new(ErrorKind::SerializeError, "invalid timestamp", None))?;
        write!(buf, "{}", naive_dt.format("%Y-%m-%dT%H:%M:%S%.f")).unwrap();
    }
    Ok(())
}

/// Format a Time64 (microseconds since midnight) as HH:MM:SS[.ffffff].
pub fn fmt_time_micros(buf: &mut String, micros: i64) -> Result<()> {
    if micros < 0 {
        return Err(Error::new(
            ErrorKind::SerializeError,
            "time micros cannot be negative",
            None,
        ));
    }
    let total_seconds = micros / 1_000_000;
    let fractional = micros % 1_000_000;
    let hour = total_seconds / 3600;
    let minute = (total_seconds % 3600) / 60;
    let second = total_seconds % 60;
    if hour > 23 || minute > 59 || second > 59 {
        return Err(Error::new(
            ErrorKind::SerializeError,
            "time micros out of range",
            None,
        ));
    }
    write!(buf, "{hour:02}:{minute:02}:{second:02}").unwrap();
    if fractional > 0 {
        let mut frac = format!("{fractional:06}");
        while frac.ends_with('0') {
            frac.pop();
        }
        buf.push('.');
        buf.push_str(&frac);
    }
    Ok(())
}

fn fmt_decimal_scaled_str(buf: &mut String, raw: &str, scale: i8) {
    let negative = raw.starts_with('-');
    let digits = if negative { &raw[1..] } else { raw };
    if scale <= 0 {
        if negative {
            buf.push('-');
        }
        buf.push_str(digits);
        buf.push('m');
        return;
    }

    let scale = scale as usize;
    let split = digits.len().saturating_sub(scale);
    let (int_part, frac_part) = if digits.len() > scale {
        (&digits[..split], &digits[split..])
    } else {
        ("0", digits)
    };

    if negative {
        buf.push('-');
    }
    buf.push_str(int_part);
    buf.push('.');
    if digits.len() <= scale {
        for _ in 0..(scale - digits.len()) {
            buf.push('0');
        }
    }
    buf.push_str(frac_part);
    buf.push('m');
}

/// Format a Decimal128 value with the given scale as a decimal string (e.g. "12.34m").
pub fn fmt_decimal128(buf: &mut String, value: i128, scale: i8) {
    let raw = value.to_string();
    fmt_decimal_scaled_str(buf, &raw, scale);
}

/// Format a Decimal256 value with the given scale as a decimal string.
pub fn fmt_decimal256(buf: &mut String, raw: &str, scale: i8) {
    fmt_decimal_scaled_str(buf, raw, scale);
}

/// Encode bytes as hex string and write to buffer.
pub fn encode_hex(buf: &mut String, v: &[u8]) {
    const HEX_LOWER: &[u8; 16] = b"0123456789abcdef";
    for &byte in v {
        buf.push(HEX_LOWER[(byte >> 4) as usize] as char);
        buf.push(HEX_LOWER[(byte & 0x0F) as usize] as char);
    }
}
