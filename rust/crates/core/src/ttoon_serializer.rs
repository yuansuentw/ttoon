//! T-TOON 結構格式序列化器（indentation-based output）
//!
//! 將 IR Node 序列化為 T-TOON 結構格式或 T-TOON 表格格式文字。

use indexmap::IndexMap;
use std::fmt::Write as FmtWrite;

use super::ir::Node;
use super::typed_fmt;
use super::{Delimiter, Error, ErrorKind, Result, TtoonOptions};

// ─── Public API ───────────────────────────────────────────────────────────────

/// 將 IR Node 序列化為 T-TOON 結構格式文字
pub fn serialize_to_ttoon_structure(node: &Node, opts: &TtoonOptions) -> Result<String> {
    let mut buf = String::with_capacity(256);
    let indent = opts.indent_size as usize;
    match node {
        Node::Object(map) => serialize_object(map, 0, indent, opts, &mut buf)?,
        Node::List(items) => serialize_root_list(items, 0, indent, opts, &mut buf)?,
        _ => {
            write_scalar(node, opts, &mut buf)?;
            buf.push_str("\n");
        }
    }
    Ok(buf)
}

// ─── Structure Serialization ──────────────────────────────────────────────────

fn serialize_object(
    map: &IndexMap<String, Node>,
    depth: usize,
    indent: usize,
    opts: &TtoonOptions,
    buf: &mut String,
) -> Result<()> {
    for (key, value) in map {
        write_indent(buf, depth, indent);
        serialize_entry(key, value, depth, indent, opts, buf)?;
    }
    Ok(())
}

/// 序列化一個 key-value 對，調用前 indent 已寫入
fn serialize_entry(
    key: &str,
    value: &Node,
    depth: usize,
    indent: usize,
    opts: &TtoonOptions,
    buf: &mut String,
) -> Result<()> {
    match value {
        Node::Object(inner) => {
            write_key(key, buf)?;
            buf.push(':');
            buf.push_str("\n");
            serialize_object(inner, depth + 1, indent, opts, buf)?;
        }
        Node::List(items) => {
            serialize_list_header_and_items(key, items, depth, indent, opts, buf)?;
        }
        _ => {
            write_key(key, buf)?;
            buf.push_str(": ");
            write_scalar(value, opts, buf)?;
            buf.push_str("\n");
        }
    }
    Ok(())
}

/// 序列化一個 list value（調用前 indent 已寫入，此函式負責寫 key+header+items）
fn serialize_list_header_and_items(
    key: &str,
    items: &[Node],
    depth: usize,
    indent: usize,
    opts: &TtoonOptions,
    buf: &mut String,
) -> Result<()> {
    let count = items.len();
    let delim = opts.delimiter;

    // 1. 嵌入式表格: 所有 items 均為相同 key 的 Object 且 values 為純量
    if let Some(fields) = extract_uniform_object_fields(items) {
        let delim_sym = delim_sym(delim);
        let delim_join = delim_join(delim);
        let delim_cell = delim_cell(delim);
        write_key(key, buf)?;
        write!(
            buf,
            "[{}{}]{{{}}}",
            count,
            delim_sym,
            fields.join(delim_join)
        )
        .unwrap();
        buf.push(':');
        buf.push_str("\n");
        for item in items {
            if let Node::Object(obj) = item {
                write_indent(buf, depth + 1, indent);
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(delim_cell);
                    }
                    match obj.get(field) {
                        Some(v) => write_scalar(v, opts, buf)?,
                        None => buf.push_str("null"),
                    }
                }
                buf.push_str("\n");
            }
        }
        return Ok(());
    }

    // 2. 內聯陣列: 所有 items 為純量（或 empty）
    if items.is_empty() || items.iter().all(is_primitive) {
        let delim_cell = delim_cell(delim);
        write_key(key, buf)?;
        write!(buf, "[{}]: ", count).unwrap();
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                buf.push_str(delim_cell);
            }
            write_scalar(item, opts, buf)?;
        }
        buf.push_str("\n");
        return Ok(());
    }

    // 3. 展開 list: key[N]:\n  - item
    write_key(key, buf)?;
    write!(buf, "[{}]:", count).unwrap();
    buf.push_str("\n");
    for item in items {
        write_indent(buf, depth + 1, indent);
        buf.push_str("- ");
        match item {
            Node::Object(inner) => {
                serialize_list_item_object(inner, depth + 1, indent, opts, buf)?;
            }
            _ => {
                write_scalar(item, opts, buf)?;
                buf.push_str("\n");
            }
        }
    }
    Ok(())
}

/// 序列化 list item 中的 object（第一個 field 在 `- ` 同行，後續 fields 在 depth+1）
fn serialize_list_item_object(
    map: &IndexMap<String, Node>,
    depth: usize,
    indent: usize,
    opts: &TtoonOptions,
    buf: &mut String,
) -> Result<()> {
    let mut iter = map.iter();

    // 第一個 field 寫在 "- " 後面
    if let Some((first_key, first_val)) = iter.next() {
        match first_val {
            Node::Object(_) | Node::List(_) => {
                write_key(first_key, buf)?;
                buf.push(':');
                buf.push_str("\n");
            }
            _ => {
                write_key(first_key, buf)?;
                buf.push_str(": ");
                write_scalar(first_val, opts, buf)?;
                buf.push_str("\n");
            }
        }
    }

    // 後續 fields 在 depth+1（對齊 "- " 後內容）
    for (k, v) in iter {
        write_indent(buf, depth + 1, indent);
        match v {
            Node::Object(inner) => {
                write_key(k, buf)?;
                buf.push(':');
                buf.push_str("\n");
                serialize_object(inner, depth + 2, indent, opts, buf)?;
            }
            Node::List(items) => {
                serialize_list_header_and_items(k, items, depth + 1, indent, opts, buf)?;
            }
            _ => {
                write_key(k, buf)?;
                buf.push_str(": ");
                write_scalar(v, opts, buf)?;
                buf.push_str("\n");
            }
        }
    }
    Ok(())
}

/// Root list serialization（無 key）
/// Uniform Objects → Tabular header `[N]{fields}:`；其他使用 `- item` 格式。
/// 空 list 使用 `[]` 語法（T-TOON 結構格式無空根列表表示法，降級為 T-JSON 語法）。
fn serialize_root_list(
    items: &[Node],
    depth: usize,
    indent: usize,
    opts: &TtoonOptions,
    buf: &mut String,
) -> Result<()> {
    // 空 list：T-TOON 無原生語法，使用 T-JSON `[]`（可被兩種 parser 正確解析）
    if items.is_empty() {
        buf.push_str("[]");
        buf.push_str("\n");
        return Ok(());
    }

    // 根層級 tabular 偵測（R10）：list of uniform scalar-valued objects → tabular format
    if let Some(fields) = extract_uniform_object_fields(items) {
        let count = items.len();
        let delim = opts.delimiter;
        let delim_sym = delim_sym(delim);
        let delim_join = delim_join(delim);
        let delim_cell = delim_cell(delim);
        write_indent(buf, depth, indent);
        write!(
            buf,
            "[{}{}]{{{}}}",
            count,
            delim_sym,
            fields.join(delim_join)
        )
        .unwrap();
        buf.push(':');
        buf.push_str("\n");
        for item in items {
            if let Node::Object(obj) = item {
                write_indent(buf, depth, indent);
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(delim_cell);
                    }
                    match obj.get(field) {
                        Some(v) => write_scalar(v, opts, buf)?,
                        None => buf.push_str("null"),
                    }
                }
                buf.push_str("\n");
            }
        }
        return Ok(());
    }

    // 內聯陣列：所有 items 為純量
    if items.iter().all(is_primitive) {
        let count = items.len();
        let delim = opts.delimiter;
        let delim_cell = delim_cell(delim);
        write_indent(buf, depth, indent);
        write!(buf, "[{}]: ", count).unwrap();
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                buf.push_str(delim_cell);
            }
            write_scalar(item, opts, buf)?;
        }
        buf.push_str("\n");
        return Ok(());
    }

    // 展開 list with "- item" format
    for item in items {
        write_indent(buf, depth, indent);
        buf.push_str("- ");
        match item {
            Node::Object(inner) => {
                serialize_list_item_object(inner, depth, indent, opts, buf)?;
            }
            _ => {
                write_scalar(item, opts, buf)?;
                buf.push_str("\n");
            }
        }
    }
    Ok(())
}

// ─── Tabular Serialization ────────────────────────────────────────────────────

// ─── Scalar Writing ───────────────────────────────────────────────────────────

pub(crate) fn write_scalar(node: &Node, opts: &TtoonOptions, buf: &mut String) -> Result<()> {
    match node {
        Node::Null => buf.push_str("null"),
        Node::Bool(v) => buf.push_str(if *v { "true" } else { "false" }),
        Node::Int(v) => typed_fmt::fmt_int(buf, *v),
        Node::Float(v) => typed_fmt::fmt_float(buf, *v),
        Node::Decimal(v) => buf.push_str(v),
        Node::String(v) => write_string(v, buf)?,
        Node::Date(v) => buf.push_str(v),
        Node::Time(v) => buf.push_str(v),
        Node::DateTime(v) => typed_fmt::fmt_datetime(buf, v),
        Node::Uuid(v) => typed_fmt::fmt_uuid(buf, v),
        Node::Binary(bytes) => typed_fmt::fmt_binary(buf, bytes, opts.binary_format)?,
        Node::List(_) | Node::Object(_) => {
            return Err(Error::new(
                ErrorKind::SerializeError,
                "cannot serialize structural node as scalar value",
                None,
            ));
        }
    }
    Ok(())
}

// ─── String Quoting ───────────────────────────────────────────────────────────

fn write_string(s: &str, buf: &mut String) -> Result<()> {
    typed_fmt::fmt_ttoon_string(buf, s)
}

// ─── Indentation / Key Helpers ────────────────────────────────────────────────

fn write_indent(buf: &mut String, depth: usize, indent_size: usize) {
    let spaces = depth * indent_size;
    for _ in 0..spaces {
        buf.push(' ');
    }
}

fn write_key(key: &str, buf: &mut String) -> Result<()> {
    if is_simple_key(key) {
        buf.push_str(key);
    } else {
        buf.push('"');
        typed_fmt::escape_ttoon_string(buf, key)?;
        buf.push('"');
    }
    Ok(())
}

fn is_simple_key(key: &str) -> bool {
    if key.is_empty() {
        return false;
    }
    let mut chars = key.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_primitive(n: &Node) -> bool {
    !matches!(n, Node::List(_) | Node::Object(_))
}

fn extract_uniform_object_fields(items: &[Node]) -> Option<Vec<String>> {
    if items.is_empty() {
        return None;
    }
    let first_fields: Vec<String> = if let Node::Object(map) = &items[0] {
        if map.is_empty() || !map.values().all(is_primitive) {
            return None;
        }
        map.keys().cloned().collect()
    } else {
        return None;
    };

    for item in items.iter().skip(1) {
        if let Node::Object(map) = item {
            let keys: Vec<String> = map.keys().cloned().collect();
            if keys != first_fields || !map.values().all(is_primitive) {
                return None;
            }
        } else {
            return None;
        }
    }
    Some(first_fields)
}

// ─── Delimiter Helpers ────────────────────────────────────────────────────────

/// bracket 內的 delimiter 標記符（TOON spec: delimsym = HTAB / '|'）
pub(crate) fn delim_sym(delim: Delimiter) -> &'static str {
    match delim {
        Delimiter::Tab => "\t",
        Delimiter::Pipe => "|",
        _ => "",
    }
}

/// field names 之間的連接字元
pub(crate) fn delim_join(delim: Delimiter) -> &'static str {
    match delim {
        Delimiter::Tab => "\t",
        Delimiter::Pipe => "|",
        _ => ",",
    }
}

/// row 各欄位之間的分隔字元
pub(crate) fn delim_cell(delim: Delimiter) -> &'static str {
    match delim {
        Delimiter::Tab => "\t",
        Delimiter::Pipe => "|",
        _ => ", ",
    }
}
