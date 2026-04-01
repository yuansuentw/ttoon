/**
 * 格式偵測
 *
 * 首字元三分支路由：
 *   '{' → tjson
 *   '[' → 判斷 TOON header pattern（[N]{fields}: 或 [N]:）→ ttoon；否則 → tjson
 *   其他 → typed_unit
 */
export type Format = 'tjson' | 'ttoon' | 'typed_unit';

export function detectFormat(input: string): Format {
  const trimmed = input.replace(/^\uFEFF/, '').trim();
  if (trimmed.length === 0) return 'typed_unit';

  const firstLine = (trimmed.split('\n')[0] ?? '').trim();
  const firstChar = firstLine[0];

  if (firstChar === '{') return 'tjson';
  if (firstChar === '[') {
    // TOON header:
    // - numeric branch: [digits...] 後接 optional {fields} 再接 :
    // - streaming branch: [*]{fields}:
    //
    // 注意：
    // - numeric branch 僅檢查首字元為 digit，不驗證是否合法數字
    // - '*' branch 只在 root tabular header (`{fields}:`) 時路由為 T-TOON
    const bracketEnd = firstLine.indexOf(']');
    if (bracketEnd < 1) return 'tjson';
    const inside = firstLine.slice(1, bracketEnd);
    const after = firstLine.slice(bracketEnd + 1).trimStart();
    if (inside.length === 0) return 'tjson';

    let hasTabularColon: boolean;
    if (after.startsWith('{')) {
      const braceEnd = after.indexOf('}');
      hasTabularColon = braceEnd >= 0 && after.slice(braceEnd + 1).trimStart().startsWith(':');
    } else {
      hasTabularColon = after.startsWith(':');
    }

    if (/^\d/.test(inside) && hasTabularColon) {
      return 'ttoon';
    }
    if (inside.startsWith('*') && after.startsWith('{') && hasTabularColon) {
      return 'ttoon';
    }
    return 'tjson';
  }
  return 'typed_unit';
}
