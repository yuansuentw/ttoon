/**
 * 值層解析 SSOT — 對應 Rust typed_parse.rs
 *
 * 此模組不依賴 Tokenizer，完全自足。
 *
 * 公開入口：
 *   parseUnit            — 解析 typed unit 字串（T-TOON cell / T-JSON typed token）
 *   parseNumberLike      — 解析 Number token 字串（值層解析 helper / 測試參考實作）
 *   validateUuidContent  — UUID 內容驗證（供 parse / stringify 共用）
 */
import { parseError } from './errors.js';
import {
  IrNode,
  irNull, irBool, irInt, irInt64, isValidI64, irFloat, irDecimal, irStr,
  irDate, irTime, irDatetime, irUuid, irBinary,
} from './ir.js';

// ─── Parse Mode ──────────────────────────────────────────────────────────────

/**
 * 解析模式：控制遇到無法識別的 bare token 時的行為。
 *
 * - `'strict'`：不認識的 bare token 視為錯誤（適用於 T-TOON serializer 產出的資料）
 * - `'compat'`：不認識的 bare token fallback 為字串（相容原始 TOON v3.0 的 bare string）
 */
export type ParseMode = 'strict' | 'compat';

// ─── 公開 API ─────────────────────────────────────────────────────────────────

/**
 * 解析 typed unit 字串（含引號字串、關鍵字、typed cells、數字）
 * 對應 Rust typed_parse::parse_unit
 *
 * @param mode 預設 `'strict'`：不認識的 bare token 回傳錯誤；`'compat'`：fallback 為字串
 *
 * @internal
 */
export function parseUnit(s: string, mode: ParseMode = 'strict'): IrNode {
  if (s.length === 0) return irStr('');

  // 帶引號的字串：strip 外層引號，T-TOON unescape
  if (s.startsWith('"')) {
    const inner = s.startsWith('"') && s.endsWith('"') ? s.slice(1, -1) : null;
    if (inner === null) throw parseError('unclosed quoted string');
    return irStr(unescapeTtoonString(inner));
  }

  // 關鍵字
  switch (s) {
    case 'null': return irNull();
    case 'true': return irBool(true);
    case 'false': return irBool(false);
    case 'inf':
    case '+inf': return irFloat(Infinity);
    case '-inf': return irFloat(-Infinity);
    case 'nan': return irFloat(NaN);
  }

  // UUID typed cell: uuid(...)
  if (s.startsWith('uuid(') && s.endsWith(')')) {
    const inner = s.slice(5, -1);
    validateUuidContent(inner, parseError);
    return irUuid(inner);
  }

  // Hex typed cell: hex(...)（空 hex() 代表空二進位，合法）
  if (s.startsWith('hex(') && s.endsWith(')')) {
    const inner = s.slice(4, -1);
    if (inner.length % 2 !== 0) throw parseError('invalid hex length in typed cell');
    return irBinary(decodeHex(inner));
  }

  // Base64 typed cell: b64(...)
  if (s.startsWith('b64(') && s.endsWith(')')) {
    const inner = s.slice(4, -1);
    return irBinary(decodeBase64(inner));
  }

  // DateTime: 含 'T' 且含 ':'（必須在 Date/Time 前偵測）
  if (s.includes('T') && s.includes(':')) return validateAndReturnDatetime(s);

  // Date: YYYY-MM-DD
  if (isDatePattern(s)) return validateAndReturnDate(s);

  // Time: HH:MM:SS[.fff]
  if (isTimePattern(s)) return validateAndReturnTime(s);

  // Decimal: 以 'm' 結尾，body 為數字
  if (s.length >= 2 && s.endsWith('m') && /^[+\-]?\d/.test(s)) {
    const decimal = parseDecimalToken(s);
    if (decimal) return decimal;
    throw parseError(`invalid decimal: ${s}`);
  }

  // Float: 含 '.' 或 'e'/'E'（需確認開頭為數字或正負號+數字，避免 bare string 如 "Alice" 誤入）
  if (/^[+\-]?\d/.test(s) && (s.includes('.') || s.includes('e') || s.includes('E'))) {
    validateFloatGrammar(s);
    const normalized = s.startsWith('+') ? s.slice(1) : s;
    return irFloat(parseFloat(normalized));
  }

  // Integer: 可選正負號 + 數字（可含 '_' 千分位）
  {
    const rest = (s.startsWith('+') || s.startsWith('-')) ? s.slice(1) : s;
    if (rest.length > 0 && /^[\d_]+$/.test(rest)) {
      validateIntegerGrammar(s);
      const normalized = s.replace(/_/g, '');
      const i = parseInt(normalized, 10);
      if (!isNaN(i) && Number.isSafeInteger(i)) return irInt(i);
      const big = BigInt(normalized);
      if (!isValidI64(big)) throw parseError(`integer ${s} exceeds signed i64 range`);
      return irInt64(big);
    }
  }

  // Fallback: 根據 mode 決定行為
  if (mode === 'compat') return irStr(s);
  throw parseError(`unknown bare token: ${JSON.stringify(s)}`);
}

/**
 * 解析 Number-like token 字串（含 datetime/time/date/decimal/float/integer）
 * 對應 Rust typed_parse::parse_number_like
 *
 * @internal
 */
export function parseNumberLike(s: string): IrNode {
  // DateTime: 含 'T' 且含 ':'（必須在 Date/Time 前偵測）
  if (s.includes('T') && s.includes(':')) return validateAndReturnDatetime(s);

  // Time: HH:MM:SS[.fff]
  if (isTimePattern(s)) return validateAndReturnTime(s);

  // Date: YYYY-MM-DD
  if (isDatePattern(s)) return validateAndReturnDate(s);

  // Decimal: 以 'm' 結尾
  if (s.endsWith('m')) {
    const decimal = parseDecimalToken(s);
    if (decimal) return decimal;
    throw parseError(`invalid decimal: ${s}`);
  }

  // Float: 含 '.' 或 'e'/'E'（含此符號的字串必須是合法 float，不允許 fall-through）
  if (s.includes('.') || s.includes('e') || s.includes('E')) {
    validateFloatGrammar(s);
    const f = Number(s.startsWith('+') ? s.slice(1) : s);
    if (!isNaN(f)) return irFloat(f);
    throw parseError(`invalid float: ${s}`);
  }

  // Integer
  validateIntegerGrammar(s);
  const i = parseInt(s.replace(/_/g, ''), 10);
  if (!isNaN(i) && Number.isSafeInteger(i)) return irInt(i);
  const big = BigInt(s.replace(/_/g, ''));
  if (!isValidI64(big)) throw parseError(`integer ${s} exceeds signed i64 range`);
  return irInt64(big);
}

/** @internal 驗證 UUID 內容：36 字元、8-4-4-4-12 小寫 hex + 連字號格式。 */
export function validateUuidContent(s: string, makeError: (msg: string) => Error): void {
  if (s.length !== 36) throw makeError('invalid uuid length in typed cell');

  if (s[8] !== '-' || s[13] !== '-' || s[18] !== '-' || s[23] !== '-') {
    throw makeError('invalid uuid format: missing hyphens');
  }

  for (let i = 0; i < s.length; i++) {
    const ch = s[i]!;
    if (i === 8 || i === 13 || i === 18 || i === 23) {
      continue;
    }
    if (!(/[0-9a-f]/.test(ch))) {
      throw makeError('invalid uuid: must be lowercase hex');
    }
  }
}

// ─── Unescape ─────────────────────────────────────────────────────────────────

/**
 * T-TOON 字串 unescape（TOON v3.0 規範：僅允許 5 種 escape）
 * 輸入為引號內的 raw 內容（不含外層 `"`）
 * 其他任何 escape sequence 均拋錯（MUST reject）
 *
 * @internal
 */
function unescapeTtoonString(raw: string): string {
  let result = '';
  let i = 0;
  while (i < raw.length) {
    if (raw[i] === '\\') {
      i++;
      const c = raw[i];
      switch (c) {
        case '\\': result += '\\'; break;
        case '"':  result += '"';  break;
        case 'n':  result += '\n'; break;
        case 'r':  result += '\r'; break;
        case 't':  result += '\t'; break;
        default:
          throw parseError(`invalid escape sequence in T-TOON string: \\${c ?? ''}`);
      }
    } else {
      result += raw[i];
    }
    i++;
  }
  return result;
}

// ─── Pattern Helpers ──────────────────────────────────────────────────────────

function isDatePattern(s: string): boolean {
  return s.length === 10 && /^\d{4}-\d{2}-\d{2}$/.test(s);
}

function isTimePattern(s: string): boolean {
  if (s.length < 8) return false;
  return s[2] === ':' && s[5] === ':';
}

// ─── Validate helpers（對齊 Rust typed_parse 驗證邏輯）────────────────────────

function daysInMonth(year: number, month: number): number {
  if (month === 2) {
    const leap = (year % 4 === 0 && year % 100 !== 0) || (year % 400 === 0);
    return leap ? 29 : 28;
  }
  return [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month] ?? 0;
}

function validateAndReturnDate(s: string): IrNode {
  const year = parseInt(s.slice(0, 4), 10);
  const month = parseInt(s.slice(5, 7), 10);
  const day = parseInt(s.slice(8, 10), 10);
  if (month < 1 || month > 12) throw parseError(`invalid date: month out of range: ${s}`);
  if (day < 1 || day > daysInMonth(year, month)) throw parseError(`invalid date: day out of range: ${s}`);
  return irDate(s);
}

function validateAndReturnTime(s: string): IrNode {
  const hour = parseInt(s.slice(0, 2), 10);
  const minute = parseInt(s.slice(3, 5), 10);
  const second = parseInt(s.slice(6, 8), 10);
  if (hour > 23) throw parseError(`invalid time: hour out of range: ${s}`);
  if (minute > 59) throw parseError(`invalid time: minute out of range: ${s}`);
  if (second > 59) throw parseError(`invalid time: second out of range: ${s}`);
  if (s.length > 8) {
    if (s[8] !== '.') throw parseError(`invalid time: expected '.' for fractional seconds: ${s}`);
    const frac = s.slice(9);
    if (frac.length === 0) throw parseError(`invalid time: empty fractional seconds: ${s}`);
    if (frac.length > 6) throw parseError(`invalid time: fractional seconds exceed 6 digits: ${s}`);
    if (!/^\d+$/.test(frac)) throw parseError(`invalid time: fractional seconds must be digits: ${s}`);
  }
  return irTime(s);
}

function validateAndReturnDatetime(s: string): IrNode {
  const tIdx = s.indexOf('T');
  if (tIdx === -1) throw parseError(`invalid datetime: ${s}`);
  const datePart = s.slice(0, tIdx);
  if (!isDatePattern(datePart)) throw parseError(`invalid datetime: invalid date part: ${s}`);
  validateAndReturnDate(datePart); // validates month/day

  const timeAndTz = s.slice(tIdx + 1);

  // Split time body from timezone
  const tzIdx = timeAndTz.search(/[Z+\-]/);
  const timeBody = tzIdx === -1 ? timeAndTz : timeAndTz.slice(0, tzIdx);
  const tzPart = tzIdx === -1 ? '' : timeAndTz.slice(tzIdx);

  // Validate time part
  if (timeBody.length < 8 || timeBody[2] !== ':' || timeBody[5] !== ':')
    throw parseError(`invalid datetime: invalid time part: ${s}`);
  const th = parseInt(timeBody.slice(0, 2), 10);
  const tm = parseInt(timeBody.slice(3, 5), 10);
  const ts = parseInt(timeBody.slice(6, 8), 10);
  if (th > 23) throw parseError(`invalid datetime: hour out of range: ${s}`);
  if (tm > 59) throw parseError(`invalid datetime: minute out of range: ${s}`);
  if (ts > 59) throw parseError(`invalid datetime: second out of range: ${s}`);
  if (timeBody.length > 8) {
    if (timeBody[8] !== '.') throw parseError(`invalid datetime: expected '.' for fractional seconds: ${s}`);
    const frac = timeBody.slice(9);
    if (frac.length === 0 || frac.length > 6 || !/^\d+$/.test(frac))
      throw parseError(`invalid datetime: invalid fractional seconds: ${s}`);
  }

  // Validate timezone
  if (tzPart.length > 0 && tzPart !== 'Z') {
    if (tzPart.length !== 6) throw parseError(`invalid datetime: invalid timezone: ${s}`);
    if (tzPart[0] !== '+' && tzPart[0] !== '-') throw parseError(`invalid datetime: invalid timezone sign: ${s}`);
    if (tzPart[3] !== ':') throw parseError(`invalid datetime: invalid timezone format: ${s}`);
    const tzH = parseInt(tzPart.slice(1, 3), 10);
    const tzM = parseInt(tzPart.slice(4, 6), 10);
    if (tzH > 23) throw parseError(`invalid datetime: timezone hour out of range: ${s}`);
    if (tzM > 59) throw parseError(`invalid datetime: timezone minute out of range: ${s}`);
  }

  return irDatetime(s);
}

function parseDecimalToken(s: string): IrNode | undefined {
  if (s.length < 2 || !s.endsWith('m')) return undefined;

  const body = s.slice(0, -1);
  const sign = body.startsWith('+') || body.startsWith('-') ? body[0] : '';
  const unsigned = sign ? body.slice(1) : body;

  // Decimal grammar: digits with optional fractional part; no trailing dot.
  if (!/^\d+(?:\.\d+)?$/.test(unsigned)) return undefined;

  const dot = unsigned.indexOf('.');
  const intPart = dot === -1 ? unsigned : unsigned.slice(0, dot);

  // Reject leading zero in integer part: 01m, 001.5m, +01m
  if (intPart.length > 1 && intPart.startsWith('0')) {
    throw parseError(`invalid decimal leading zero: ${s}`);
  }

  // Reject negative zero forms: -0m, -0.0m, -0.00m
  if (sign === '-' && /^0(?:\.0+)?$/.test(unsigned)) {
    throw parseError(`invalid decimal negative zero: ${s}`);
  }

  return irDecimal(s);
}

// ─── Number Grammar Validation (aligned with Rust typed_parse) ───────────────

/** Validate integer grammar: no leading zeros, -0 rejected, _ grouping must be 1-3 + 3-digit groups */
function validateIntegerGrammar(s: string): void {
  const sign = (s[0] === '+' || s[0] === '-') ? s[0] : null;
  const digits = sign ? s.slice(1) : s;

  if (digits.length === 0) throw parseError(`invalid integer: ${s}`);

  if (digits === '0') {
    if (sign === '-') throw parseError('integer cannot be -0');
    return;
  }

  if (digits.startsWith('0')) {
    throw parseError('integer cannot have leading zero');
  }

  if (digits.includes('_')) {
    const parts = digits.split('_');
    if (parts.length < 2) throw parseError('invalid integer grouping');
    const first = parts[0]!;
    if (first.length === 0 || first.length > 3) throw parseError('invalid integer grouping');
    if (!/^\d+$/.test(first) || first.startsWith('0')) throw parseError('invalid integer grouping');
    for (let i = 1; i < parts.length; i++) {
      const part = parts[i]!;
      if (part.length !== 3 || !/^\d{3}$/.test(part)) throw parseError('invalid integer grouping');
    }
  } else {
    if (!/^\d+$/.test(digits)) throw parseError(`invalid integer: ${s}`);
  }
}

/** Validate float grammar: no _, no uppercase E, dot requires both sides, exponent needs digits */
function validateFloatGrammar(s: string): void {
  if (s.includes('_') || s.includes('E')) throw parseError(`invalid float: ${s}`);

  const eParts = s.split('e');
  if (eParts.length > 2) throw parseError(`invalid float: ${s}`);

  const base = eParts[0]!;
  if (base.length === 0) throw parseError(`invalid float: ${s}`);

  const baseSign = (base[0] === '+' || base[0] === '-') ? base[0] : null;
  const baseDigits = baseSign ? base.slice(1) : base;
  if (baseDigits.length === 0) throw parseError(`invalid float: ${s}`);

  if (baseDigits.includes('.')) {
    const dotParts = baseDigits.split('.');
    if (dotParts.length !== 2) throw parseError(`invalid float: ${s}`);
    const intPart = dotParts[0]!;
    const fracPart = dotParts[1]!;
    if (intPart.length === 0 || fracPart.length === 0) throw parseError(`invalid float: ${s}`);
    if (!isFloatInt(intPart)) throw parseError(`invalid float: ${s}`);
    if (!/^\d+$/.test(fracPart)) throw parseError(`invalid float: ${s}`);
  } else {
    if (!isFloatInt(baseDigits)) throw parseError(`invalid float: ${s}`);
  }

  if (eParts.length === 2) {
    const exp = eParts[1]!;
    if (exp.length === 0) throw parseError(`invalid float: ${s}`);
    const expSign = (exp[0] === '+' || exp[0] === '-') ? exp[0] : null;
    const expDigits = expSign ? exp.slice(1) : exp;
    if (expDigits.length === 0 || !/^\d+$/.test(expDigits)) throw parseError(`invalid float: ${s}`);
  }
}

function isFloatInt(s: string): boolean {
  if (s === '0') return true;
  if (s.length === 0 || s.startsWith('0')) return false;
  return /^\d+$/.test(s);
}

// ─── Binary Decoding ──────────────────────────────────────────────────────────

function decodeHex(s: string): Uint8Array {
  if (!/^[0-9A-Fa-f]*$/.test(s)) throw parseError('invalid hex character');
  const bytes = new Uint8Array(s.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    const byte = parseInt(s.slice(i * 2, i * 2 + 2), 16);
    bytes[i] = byte;
  }
  return bytes;
}

function decodeBase64(s: string): Uint8Array {
  try {
    const binary = atob(s);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    return bytes;
  } catch {
    throw parseError('invalid base64 in typed cell');
  }
}
