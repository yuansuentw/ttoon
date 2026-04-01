//! T-TOON 結構格式解析器（indentation-based line parser）
//!
//! 解析 T-TOON 結構格式（縮排式 key-value / list / nested object）
//! 以及 T-TOON 表格格式（`[N]{fields}:` header）。

use indexmap::IndexMap;

use super::ir::Node;
use super::typed_parse::ParseMode;
use super::{Delimiter, Error, ErrorKind, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TabularRowCount {
    Exact(usize),
    Streaming,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TabularHeader {
    pub row_count: TabularRowCount,
    pub delimiter: Delimiter,
    pub fields: Vec<String>,
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// 從 T-TOON Tabular 格式的 header 行提取欄位名稱（保留原始順序）。
/// 若輸入非 tabular 格式或 header 無明確欄位名稱，回傳 None。
pub(crate) fn extract_tabular_fields(input: &str) -> Option<Vec<String>> {
    let trimmed = input.trim_start_matches('\u{FEFF}').trim();
    let first_line = trimmed.lines().next()?;
    let first_trimmed = first_line.trim();
    if !first_trimmed.starts_with('[') || !first_trimmed.ends_with(':') {
        return None;
    }
    if let Ok(header) = parse_root_tabular_header(first_trimmed) {
        let fields = header.fields;
        if !fields.is_empty() {
            return Some(fields);
        }
    }
    None
}

/// 自動偵測並解析 T-TOON 輸入（tabular 或 structure），回傳 IR Node
/// `mode` 預設 `Compat`：未知 bare token 視為字串（符合 T-TOON spec §2.1 第 12 項）。
/// 傳入 `Strict` 可啟用嚴格驗證，拒絕任何不符合已知型別的 bare token。
pub fn parse_ttoon(input: &str, mode: ParseMode) -> Result<Node> {
    let trimmed = input.trim_start_matches('\u{FEFF}').trim();
    let first_line = trimmed.lines().next().unwrap_or("");
    let first_trimmed = first_line.trim();
    // 偵測根層級 inline array（[N]: v1, v2, ...）
    if let Some(result) = try_parse_root_inline_array(first_trimmed, mode) {
        return result;
    }
    // 偵測根層級 tabular header（`[N]{fields}:` 或 `[N]:`）
    if first_trimmed.starts_with('[') && first_trimmed.ends_with(':') {
        return parse_ttoon_tabular(input, mode);
    }

    let non_empty_lines: Vec<&str> = trimmed
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    if non_empty_lines.iter().any(|raw_line| {
        let top_level = raw_line.len() == raw_line.trim_start_matches(' ').len();
        let line = raw_line.trim();
        top_level
            && !line.starts_with("- ")
            && find_array_header_bracket(line).is_none()
            && !has_colon_kv(line)
            && has_removed_line_separated_syntax(line)
    }) {
        return Err(Error::new(
            ErrorKind::ParseError,
            "line-separated rows are not supported; use T-JSON arrays or T-TOON tabular syntax",
            None,
        ));
    }
    if non_empty_lines.len() == 1 {
        let single_line = non_empty_lines[0].trim();
        if !single_line.starts_with("- ")
            && find_array_header_bracket(single_line).is_none()
            && !has_colon_kv(single_line)
        {
            return parse_typed_unit_to_node(single_line, mode);
        }
    }
    parse_ttoon_structure(input, mode)
}

/// Local adapter for the Node-building T-TOON parser.
fn parse_typed_unit_to_node(raw: &str, mode: ParseMode) -> Result<Node> {
    crate::typed_parse::parse_unit_typed_value(raw, mode).map(Into::into)
}

/// 解析 T-TOON 結構格式文字，回傳 IR Node（Object 或 List）
pub fn parse_ttoon_structure(input: &str, mode: ParseMode) -> Result<Node> {
    let indent_size = 2usize;
    let lines = collect_lines(input, indent_size);
    if lines.is_empty() {
        return Ok(Node::Object(IndexMap::new()));
    }
    let mut pos = 0usize;
    parse_block(&lines, &mut pos, 0, indent_size, mode)
}

/// 解析 T-TOON 表格格式文字，回傳 IR Node（List of Objects）
pub(crate) fn parse_ttoon_tabular(input: &str, mode: ParseMode) -> Result<Node> {
    let mut lines_iter = input.lines();

    // 找到第一個非空行作為 header
    let header_line = loop {
        match lines_iter.next() {
            None => return Ok(Node::List(Vec::new())),
            Some(l) if l.trim().is_empty() => continue,
            Some(l) => break l.trim(),
        }
    };

    let header = parse_root_tabular_header(header_line)?;
    let delim = header.delimiter;
    let fields = header.fields;

    let mut rows: Vec<Node> = Vec::new();
    for raw_line in lines_iter {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        let vals = split_row_quote_aware(line, &delim)?;
        if !fields.is_empty() && vals.len() != fields.len() {
            return Err(Error::new(
                ErrorKind::ParseError,
                format!("row has {} fields, expected {}", vals.len(), fields.len()),
                None,
            ));
        }

        if fields.is_empty() {
            let row_nodes: Result<Vec<Node>> = vals
                .iter()
                .map(|v| parse_typed_unit_to_node(v.trim(), mode))
                .collect();
            rows.push(Node::List(row_nodes?));
        } else {
            let mut obj = IndexMap::new();
            for (field, val_str) in fields.iter().zip(vals.iter()) {
                obj.insert(
                    field.clone(),
                    parse_typed_unit_to_node(val_str.trim(), mode)?,
                );
            }
            rows.push(Node::Object(obj));
        }
    }

    match header.row_count {
        TabularRowCount::Exact(expected) if rows.len() != expected => {
            return Err(Error::new(
                ErrorKind::ParseError,
                format!(
                    "tabular row count mismatch: header declares {} rows, got {}",
                    expected,
                    rows.len()
                ),
                None,
            ))
        }
        TabularRowCount::Streaming if rows.is_empty() => {
            return Err(Error::new(
                ErrorKind::ParseError,
                "tabular [*] header requires at least one data row",
                None,
            ))
        }
        _ => {}
    }

    Ok(Node::List(rows))
}

// ─── Line Collection ─────────────────────────────────────────────────────────

struct Line<'a> {
    depth: usize,
    content: &'a str,
}

fn collect_lines<'a>(input: &'a str, indent_size: usize) -> Vec<Line<'a>> {
    input
        .lines()
        .filter_map(|raw| {
            if raw.trim().is_empty() {
                return None;
            }
            let leading = raw.len() - raw.trim_start_matches(' ').len();
            let depth = if indent_size > 0 {
                leading / indent_size
            } else {
                0
            };
            let content = &raw[leading..];
            Some(Line { depth, content })
        })
        .collect()
}

// ─── Block / Object / List Parsing ───────────────────────────────────────────

fn parse_block(
    lines: &[Line<'_>],
    pos: &mut usize,
    depth: usize,
    indent_size: usize,
    mode: ParseMode,
) -> Result<Node> {
    if let Some(line) = lines.get(*pos) {
        if line.depth == depth && line.content.starts_with("- ") {
            return parse_list_items(lines, pos, depth, indent_size, mode);
        }
    }
    parse_object(lines, pos, depth, indent_size, mode)
}

fn parse_object(
    lines: &[Line<'_>],
    pos: &mut usize,
    depth: usize,
    indent_size: usize,
    mode: ParseMode,
) -> Result<Node> {
    let mut map = IndexMap::new();
    while let Some(line) = lines.get(*pos) {
        if line.depth != depth {
            break;
        }
        let content = line.content;
        *pos += 1;
        let (key, value) = parse_kv_line(content, lines, pos, depth, indent_size, mode)?;
        map.insert(key, value);
    }
    Ok(Node::Object(map))
}

/// 解析以 `- ` 開頭的 list items，depth 為 `- ` 所在深度
fn parse_list_items(
    lines: &[Line<'_>],
    pos: &mut usize,
    depth: usize,
    indent_size: usize,
    mode: ParseMode,
) -> Result<Node> {
    let mut items = Vec::new();
    while let Some(line) = lines.get(*pos) {
        if line.depth != depth || !line.content.starts_with("- ") {
            break;
        }
        let content = &line.content[2..]; // strip "- "
        *pos += 1;

        if has_colon_kv(content) {
            let item = parse_list_item_object(content, lines, pos, depth, indent_size, mode)?;
            items.push(item);
        } else {
            items.push(parse_typed_unit_to_node(content.trim(), mode)?);
        }
    }
    Ok(Node::List(items))
}

/// 解析 list item 中的 object（第一個 field 在 `- ` 同行，後續 fields 在 depth+1）
fn parse_list_item_object(
    first_kv: &str,
    lines: &[Line<'_>],
    pos: &mut usize,
    depth: usize,
    indent_size: usize,
    mode: ParseMode,
) -> Result<Node> {
    let mut map = IndexMap::new();
    let (key, value) = parse_kv_line(first_kv, lines, pos, depth + 1, indent_size, mode)?;
    map.insert(key, value);
    // 後續 fields 在 depth+1（與 "- " 後的內容對齊）
    while let Some(line) = lines.get(*pos) {
        if line.depth != depth + 1 || line.content.starts_with("- ") {
            break;
        }
        let content = line.content;
        *pos += 1;
        let (k, v) = parse_kv_line(content, lines, pos, depth + 1, indent_size, mode)?;
        map.insert(k, v);
    }
    Ok(Node::Object(map))
}

// ─── Key-Value Line Parsing ───────────────────────────────────────────────────

fn parse_kv_line(
    line: &str,
    lines: &[Line<'_>],
    pos: &mut usize,
    depth: usize,
    indent_size: usize,
    mode: ParseMode,
) -> Result<(String, Node)> {
    // Array header: key[N...]:...
    if let Some(bracket_pos) = find_array_header_bracket(line) {
        return parse_array_header_line(line, bracket_pos, lines, pos, depth, indent_size, mode);
    }

    // Regular: key: value 或 key: (nested)
    if let Some(sep) = find_colon_space(line) {
        let key = parse_key(&line[..sep])?;
        let val_str = line[sep + 2..].trim();
        if val_str.is_empty() {
            let nested = parse_block(lines, pos, depth + 1, indent_size, mode)?;
            return Ok((key, nested));
        }
        return Ok((key, parse_typed_unit_to_node(val_str, mode)?));
    }

    // Trailing colon: key: (nested object)
    if let Some(key_part) = line.strip_suffix(':') {
        let key_part = key_part.trim();
        if !key_part.is_empty() && !key_part.ends_with(':') {
            let key = parse_key(key_part)?;
            let nested = parse_block(lines, pos, depth + 1, indent_size, mode)?;
            return Ok((key, nested));
        }
    }

    Err(Error::new(
        ErrorKind::ParseError,
        format!("invalid T-TOON structure line: {:?}", line),
        None,
    ))
}

fn parse_array_header_line(
    line: &str,
    bracket_pos: usize,
    lines: &[Line<'_>],
    pos: &mut usize,
    depth: usize,
    indent_size: usize,
    mode: ParseMode,
) -> Result<(String, Node)> {
    let key = parse_key(line[..bracket_pos].trim())?;
    let rest = &line[bracket_pos + 1..]; // after '['

    let close = rest
        .find(']')
        .ok_or_else(|| Error::new(ErrorKind::ParseError, "missing ']' in array header", None))?;
    let bracket_content = &rest[..close];
    let after_bracket = &rest[close + 1..]; // after ']'

    let (count, delim) = parse_bracket_segment(bracket_content)?;
    let _ = count;

    // 嵌入式表格: key[N]{fields}:
    if let Some(brace_rest) = after_bracket.strip_prefix('{') {
        let brace_end = brace_rest.find("}:").ok_or_else(|| {
            Error::new(ErrorKind::ParseError, "missing '}:' in tabular array", None)
        })?;
        let fields_str = &brace_rest[..brace_end];
        let fields: Vec<String> = split_by_delim(fields_str, &delim)
            .into_iter()
            .map(|s| s.trim().to_string())
            .collect();
        let items = parse_embedded_tabular_rows(lines, pos, depth + 1, &fields, &delim, mode)?;
        return Ok((key, Node::List(items)));
    }

    // 內聯陣列: key[N]: v1, v2
    if let Some(values_str) = after_bracket.strip_prefix(": ") {
        let fields = split_row_quote_aware(values_str.trim(), &delim)?;
        let items: Result<Vec<Node>> = fields
            .into_iter()
            .map(|s| parse_typed_unit_to_node(s.trim(), mode))
            .collect();
        return Ok((key, Node::List(items?)));
    }

    // 展開 list 或 nested object: key[N]:
    if after_bracket == ":" {
        // 先檢查展開 list items (- ...)
        if lines.get(*pos).map_or(false, |l| {
            l.depth == depth + 1 && l.content.starts_with("- ")
        }) {
            let list_node = parse_list_items(lines, pos, depth + 1, indent_size, mode)?;
            return Ok((key, list_node));
        }
        // 再檢查 nested object
        if lines.get(*pos).map_or(false, |l| l.depth == depth + 1) {
            let nested = parse_object(lines, pos, depth + 1, indent_size, mode)?;
            return Ok((key, nested));
        }
        // 空陣列
        return Ok((key, Node::List(Vec::new())));
    }

    Err(Error::new(
        ErrorKind::ParseError,
        "invalid array header syntax",
        None,
    ))
}

fn parse_embedded_tabular_rows(
    lines: &[Line<'_>],
    pos: &mut usize,
    depth: usize,
    fields: &[String],
    delim: &Delimiter,
    mode: ParseMode,
) -> Result<Vec<Node>> {
    let mut rows = Vec::new();
    while let Some(line) = lines.get(*pos) {
        if line.depth < depth {
            break;
        }
        let content = line.content;
        *pos += 1;
        let vals = split_row_quote_aware(content, delim)?;
        if vals.len() != fields.len() {
            return Err(Error::new(
                ErrorKind::ParseError,
                format!(
                    "tabular row has {} fields, expected {}",
                    vals.len(),
                    fields.len()
                ),
                None,
            ));
        }
        let mut obj = IndexMap::new();
        for (field, val_str) in fields.iter().zip(vals.iter()) {
            obj.insert(
                field.clone(),
                parse_typed_unit_to_node(val_str.trim(), mode)?,
            );
        }
        rows.push(Node::Object(obj));
    }
    Ok(rows)
}

// ─── Tabular Header Parsing ───────────────────────────────────────────────────

pub(crate) fn parse_root_tabular_header(line: &str) -> Result<TabularHeader> {
    if !line.starts_with('[') {
        return Err(Error::new(
            ErrorKind::ParseError,
            "tabular header must start with '['",
            None,
        ));
    }
    let after_bracket = &line[1..];
    let close = after_bracket
        .find(']')
        .ok_or_else(|| Error::new(ErrorKind::ParseError, "missing ']' in tabular header", None))?;
    let bracket_content = &after_bracket[..close];
    let after_close = &after_bracket[close + 1..];

    let (row_count, delimiter) = parse_tabular_bracket_segment(bracket_content)?;

    if let Some(brace_rest) = after_close.strip_prefix('{') {
        let brace_end = brace_rest.find("}:").ok_or_else(|| {
            Error::new(
                ErrorKind::ParseError,
                "missing '}:' in tabular header",
                None,
            )
        })?;
        let fields_str = &brace_rest[..brace_end];
        let fields: Vec<String> = split_by_delim(fields_str, &delimiter)
            .into_iter()
            .map(|s| s.trim().to_string())
            .collect();
        return Ok(TabularHeader {
            row_count,
            delimiter,
            fields,
        });
    }

    if after_close == ":" && !matches!(row_count, TabularRowCount::Streaming) {
        return Ok(TabularHeader {
            row_count,
            delimiter,
            fields: Vec::new(),
        });
    }

    Err(Error::new(
        ErrorKind::ParseError,
        "invalid tabular header",
        None,
    ))
}

pub(crate) fn format_root_tabular_header(
    row_count: TabularRowCount,
    fields: &[String],
    delimiter: Delimiter,
) -> String {
    let mut buf = String::with_capacity(64);
    let row_count = match row_count {
        TabularRowCount::Exact(count) => count.to_string(),
        TabularRowCount::Streaming => "*".to_string(),
    };
    let delim_sym = crate::ttoon_serializer::delim_sym(delimiter);
    let delim_join = crate::ttoon_serializer::delim_join(delimiter);

    if fields.is_empty() {
        buf.push('[');
        buf.push_str(&row_count);
        buf.push_str(delim_sym);
        buf.push_str("]:");
    } else {
        buf.push('[');
        buf.push_str(&row_count);
        buf.push_str(delim_sym);
        buf.push_str("]{");
        buf.push_str(&fields.join(delim_join));
        buf.push_str("}:");
    }

    buf
}

// ─── Delimiter ────────────────────────────────────────────────────────────────

fn split_bracket_content(content: &str) -> (&str, Delimiter) {
    if content.ends_with('\t') {
        (&content[..content.len() - 1], Delimiter::Tab)
    } else if content.ends_with("\\|") {
        (&content[..content.len() - 2], Delimiter::Pipe)
    } else if content.ends_with('|') {
        (&content[..content.len() - 1], Delimiter::Pipe)
    } else if content.ends_with(',') {
        (&content[..content.len() - 1], Delimiter::Comma)
    } else {
        (content, Delimiter::Comma)
    }
}

fn parse_tabular_bracket_segment(content: &str) -> Result<(TabularRowCount, Delimiter)> {
    let (num_str, delim) = split_bracket_content(content);
    if num_str == "*" {
        return Ok((TabularRowCount::Streaming, delim));
    }
    Ok((TabularRowCount::Exact(parse_row_count(num_str)?), delim))
}

fn parse_row_count(num_str: &str) -> Result<usize> {
    if num_str.is_empty() {
        Ok(0)
    } else {
        num_str
            .parse::<usize>()
            .map_err(|_| Error::new(ErrorKind::ParseError, "invalid row count in header", None))
    }
}

fn parse_bracket_segment(content: &str) -> Result<(usize, Delimiter)> {
    let (num_str, delim) = split_bracket_content(content);
    Ok((parse_row_count(num_str)?, delim))
}

fn split_by_delim<'a>(s: &'a str, delim: &Delimiter) -> Vec<&'a str> {
    match delim {
        Delimiter::Comma => s.split(',').collect(),
        Delimiter::Tab => s.split('\t').collect(),
        Delimiter::Pipe => s.split('|').collect(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DelimitedLexerState {
    Bare,
    Quoted,
}

/// Quote-aware row splitter for productions like `field *(delim field)`.
///
/// delimiter 只在 quoted string 外生效；值的語意解析仍委派給 typed_parse。
pub(crate) fn split_row_quote_aware<'a>(line: &'a str, delim: &Delimiter) -> Result<Vec<&'a str>> {
    let sep = match delim {
        Delimiter::Comma => b',',
        Delimiter::Tab => b'\t',
        Delimiter::Pipe => b'|',
    };
    let bytes = line.as_bytes();
    let mut fields = Vec::new();
    let mut state = DelimitedLexerState::Bare;
    let mut start = 0;
    let mut i = 0;

    while i < bytes.len() {
        let byte = bytes[i];
        if state == DelimitedLexerState::Quoted {
            if byte == b'\\' {
                i += 2;
                continue;
            }
            if byte == b'"' {
                state = DelimitedLexerState::Bare;
            }
            i += 1;
            continue;
        }

        if byte == b'"' {
            state = DelimitedLexerState::Quoted;
            i += 1;
            continue;
        }

        if byte == sep {
            fields.push(&line[start..i]);
            start = i + 1;
        }

        i += 1;
    }

    if state == DelimitedLexerState::Quoted {
        return Err(Error::new(
            ErrorKind::ParseError,
            "unclosed quoted string",
            None,
        ));
    }

    fields.push(&line[start..]);

    Ok(fields)
}

// ─── Root Inline Array ──────────────────────────────────────────────────────

/// 嘗試將 `[N]: v1, v2, ...` 格式的根層級 inline array 解析為 Node::List。
/// 若行不匹配此格式，回傳 None。
fn try_parse_root_inline_array(line: &str, mode: ParseMode) -> Option<Result<Node>> {
    if !line.starts_with('[') {
        return None;
    }
    let after = &line[1..];
    let close = after.find(']')?;
    let bracket = &after[..close];
    if bracket.is_empty() || !bracket.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let after_close = &after[close + 1..];
    // 必須是 ": <values>"（注意與 tabular `:` 結尾的區別）
    let remainder = after_close.strip_prefix(": ")?;

    let (_, delim) = match parse_bracket_segment(bracket) {
        Ok(v) => v,
        Err(e) => return Some(Err(e)),
    };

    if remainder.trim().is_empty() {
        return Some(Ok(Node::List(Vec::new())));
    }

    let fields = match split_row_quote_aware(remainder.trim(), &delim) {
        Ok(v) => v,
        Err(e) => return Some(Err(e)),
    };
    let items: Result<Vec<Node>> = fields
        .into_iter()
        .map(|s| parse_typed_unit_to_node(s.trim(), mode))
        .collect();
    match items {
        Ok(v) => Some(Ok(Node::List(v))),
        Err(e) => Some(Err(e)),
    }
}

// ─── Key Helpers ──────────────────────────────────────────────────────────────

/// 在 line 中找到 array header 的 `[` 位置（必須在第一個 `: ` 之前）
fn find_array_header_bracket(line: &str) -> Option<usize> {
    let colon_pos = find_colon_space(line).unwrap_or(usize::MAX);
    let bracket_pos = line.find('[')?;
    if bracket_pos < colon_pos {
        Some(bracket_pos)
    } else {
        None
    }
}

/// 找到第一個 `: `（colon + space）的位置
fn find_colon_space(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    for i in 0..bytes.len().saturating_sub(1) {
        if bytes[i] == b':' && bytes[i + 1] == b' ' {
            return Some(i);
        }
    }
    None
}

/// 判斷 line 是否有 key-value 語法（含 `: ` 或以 `:` 結尾）
fn has_colon_kv(s: &str) -> bool {
    if let Some(sep) = find_colon_space(s) {
        return parse_key(s[..sep].trim()).is_ok();
    }
    s.ends_with(':')
        && s.len() > 1
        && !s.ends_with("::")
        && parse_key(s[..s.len() - 1].trim()).is_ok()
}

fn has_removed_line_separated_syntax(s: &str) -> bool {
    let mut in_quotes = false;
    let mut escaped = false;
    for ch in s.chars() {
        if in_quotes {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_quotes = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' => in_quotes = true,
            ',' => return true,
            _ => {}
        }
    }
    false
}

fn parse_key(s: &str) -> Result<String> {
    let s = s.trim();
    if s.starts_with('"') {
        let inner = s
            .strip_prefix('"')
            .and_then(|t| t.strip_suffix('"'))
            .ok_or_else(|| Error::new(ErrorKind::ParseError, "invalid quoted key", None))?;
        return crate::typed_parse::unescape_ttoon_string(inner)
            .map_err(|_| Error::new(ErrorKind::ParseError, "invalid quoted key", None));
    }
    if s.is_empty() {
        return Err(Error::new(ErrorKind::ParseError, "empty key", None));
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return Err(Error::new(
            ErrorKind::ParseError,
            format!("invalid key: {:?}", s),
            None,
        ));
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(Error::new(
            ErrorKind::ParseError,
            format!("invalid key: {:?}", s),
            None,
        ));
    }
    Ok(s.to_string())
}
