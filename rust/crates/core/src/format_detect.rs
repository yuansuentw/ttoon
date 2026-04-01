/// 格式偵測結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// T-JSON（JSON-like `{}`/`[]` 語法）
    Tjson,
    /// T-TOON（TOON header：`[N]{fields}:` tabular array 或 `[N]:` list）
    Ttoon,
    /// Typed unit（純量、縮排 key-value 等非 `{`/`[` 開頭的內容）
    TypedUnit,
}

/// 首字元三分支格式偵測：
/// - `{` → Tjson
/// - `[` → 判斷 TOON header pattern（`[N]{fields}:` 或 `[N]:`）→ Ttoon；否則 → Tjson
/// - 其他 → TypedUnit
pub fn detect(input: &str) -> Format {
    let trimmed = input.trim_start_matches('\u{FEFF}').trim();
    if trimmed.is_empty() {
        return Format::TypedUnit;
    }

    let first_line = trimmed.lines().next().unwrap_or("");
    let first_trimmed = first_line.trim();

    match first_trimmed.as_bytes()[0] {
        b'{' => Format::Tjson,
        b'[' => {
            // TOON header:
            // - numeric branch: [digits...] 後接 optional {fields} 再接 :
            // - streaming branch: [*]{fields}:
            //
            // 注意：
            // - numeric branch 僅檢查首字元為 digit，不驗證是否合法數字
            // - '*' branch 只在 root tabular header (`{fields}:`) 時路由為 TTOON
            let bracket_end = match first_trimmed.find(']') {
                Some(i) => i,
                None => return Format::Tjson,
            };
            let inside = &first_trimmed[1..bracket_end];
            let after = first_trimmed[bracket_end + 1..].trim_start();
            let has_tabular_colon = if after.starts_with('{') {
                after
                    .find('}')
                    .map(|i| after[i + 1..].trim_start().starts_with(':'))
                    .unwrap_or(false)
            } else {
                after.starts_with(':')
            };

            if inside.is_empty() {
                return Format::Tjson;
            }

            if inside.as_bytes()[0].is_ascii_digit() && has_tabular_colon {
                Format::Ttoon
            } else if inside.starts_with('*') && after.starts_with('{') && has_tabular_colon {
                Format::Ttoon
            } else {
                Format::Tjson
            }
        }
        _ => Format::TypedUnit,
    }
}
