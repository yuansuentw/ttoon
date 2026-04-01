/**
 * T-TOON JavaScript/TypeScript 套件 — 公開 API
 *
 * 主要函式：
 *   parse(text)           — 解析 T-TOON/T-JSON 文字為 JS 值（自動偵測格式）
 *   stringify(value)      — 將 JS 值序列化為 T-TOON 文字
 *   initWasm(input?)      — 顯式初始化 WASM bridge（web/node ESM 先呼叫）
 *   readArrow(text)       — 解析文字為 Arrow Table（async）
 *   stringifyArrow(table)      — 將 Arrow Table 序列化為 T-TOON Tabular 文字（async）
 *   toTjson(value)             — 將 JS 值序列化為 T-JSON 文字
 *   stringifyArrowTjson(table) — 將 Arrow Table 序列化為 T-JSON 文字（async）
 *   detectFormat(t)            — 偵測格式類型（'tjson' | 'ttoon' | 'typed_unit'）
 *   use(codecs)           — 全域 Codec 注入
 *
 * 特殊型別標記（序列化時使用）：
 *   toon.uuid('...')     — UUID 字串
 *   toon.decimal('...')  — Decimal 數值
 *   toon.date('...')     — 日期
 *   toon.time('...')     — 時間
 *   toon.datetime('...') — 日期時間
 */
import type { Table as ArrowTable } from 'apache-arrow';
import {
  irToJs, jsToIr,
  type ToonInput, type ToonOutput,
} from './convert.js';
import { use as useCodecs, type Codec, type CodecRegistry } from './codec.js';
import { transcodeError } from './errors.js';
import { detectFormat as detectFormatInternal, type Format } from './format_detect.js';
import {
  type SerializeOptions,
} from './ttoon_serializer.js';
import {
  type TjsonSerializeOptions,
} from './tjson_serializer.js';
import {
  wasmParse,
  wasmStringifyTtoon,
  wasmStringifyTjson,
  wasmTjsonToTtoon,
  wasmTtoonToTjson,
  wasmReadArrow,
  wasmStringifyArrowTtoon,
  wasmStringifyArrowTjson,
} from './wasm_adapter.js';

export { isArrowTable } from './arrow_convert.js';
export {
  toon,
  type ToonInput, type ToonOutput, type ToonTagged,
} from './convert.js';
export { initWasm, isWasmInitialized } from './wasm_adapter.js';
export { type Codec, type CodecPayload, type CodecRegistry, type CodecType, type IntPayload } from './codec.js';
export {
  ToonError,
  TranscodeError,
  type ErrorKind,
  type SourceErrorKind,
  type TranscodeOperation,
  type TranscodePhase,
} from './errors.js';
export { type Format } from './format_detect.js';
export { intNumber, intBigInt } from './int_codec.js';
export { FieldTypeSpec, StreamSchema, types, type StreamSchemaInput } from './schema.js';
export {
  ArrowStreamWriter,
  StreamWriter,
  TjsonArrowStreamWriter,
  TjsonStreamWriter,
  streamRead,
  streamReadArrow,
  streamReadArrowTjson,
  streamReadTjson,
  streamWriter,
  streamWriterArrow,
  streamWriterArrowTjson,
  streamWriterTjson,
} from './streaming.js';
export type {
  StreamReadArrowOptions,
  StreamReadOptions,
  StreamResult,
  StreamWriteArrowOptions,
  StreamWriteOptions,
  TextSink,
  TextSource,
  TjsonStreamWriteArrowOptions,
  TjsonStreamWriteOptions,
} from './streaming.js';
export { type ParseMode } from './typed_parse.js';
export { type SerializeOptions } from './ttoon_serializer.js';
export { type TjsonSerializeOptions } from './tjson_serializer.js';

// ─── 解析選項 ─────────────────────────────────────────────────────────────────

export interface ParseOptions {
  /** Per-call Codec 覆寫（優先於 use() 全域註冊） */
  codecs?: CodecRegistry;
  /**
   * 解析模式（僅影響 T-TOON 格式）。
   * - `'compat'`（預設）：不認識的 bare token fallback 為字串（相容原始 TOON v3.0）
   * - `'strict'`：不認識的 bare token 視為錯誤
   */
  mode?: import('./typed_parse.js').ParseMode;
}

export interface TjsonToTtoonOptions extends SerializeOptions {}

export interface TtoonToTjsonOptions extends TjsonSerializeOptions {
  mode?: import('./typed_parse.js').ParseMode;
}

// ─── Arrow IPC utilities ────────────────────────────────────────────────────

/** Dynamically load tableToIPC / tableFromIPC from apache-arrow */
async function loadArrowIpc() {
  const arrow = await import('apache-arrow');
  return {
    tableToIPC: arrow.tableToIPC,
    tableFromIPC: arrow.tableFromIPC,
  };
}

// ─── 主要函式 ─────────────────────────────────────────────────────────────────

/**
 * 解析 T-TOON/T-JSON 文字為 JavaScript 值
 *
 * 自動偵測格式（tjson / ttoon）。
 * 支援頂層純量、object、array。
 */
export function parse<T = ToonOutput>(text: string, options?: ParseOptions): T {
  const codecOverrides = options?.codecs;
  const mode = options?.mode ?? 'compat';
  const ir = wasmParse(text, mode);
  return irToJs(ir, codecOverrides) as unknown as T;
}

/**
 * 將 JavaScript 值序列化為 T-TOON 文字
 *
 * 自動選擇輸出格式：
 *   - Object → 縮排 key-value
 *   - List of uniform Objects → Tabular 格式 [N]{fields}:
 *   - 其他 → 縮排格式
 */
export function stringify(value: ToonInput, options?: SerializeOptions): string {
  const irNode = jsToIr(value);
  return wasmStringifyTtoon(irNode, options);
}

/**
 * 將 T-JSON 文字直接轉為 T-TOON 文字，不經過 JS object 中轉
 */
export function tjsonToTtoon(text: string, options?: TjsonToTtoonOptions): string {
  try {
    return wasmTjsonToTtoon(text, options);
  } catch (error) {
    throw transcodeError('tjson_to_ttoon', 'parse', error);
  }
}

/**
 * 解析文字為 Arrow Table（async）
 *
 * 自動偵測格式，要求資料為 2D arrowable 結構，欄位值必須為純量。
 * - T-JSON list of objects：允許 sparse rows，缺 key 視為 null，schema 欄位順序以 batch 內首次出現順序推導。
 * - T-TOON tabular：以 header 欄位順序/寬度為準。
 * - T-TOON structure：不提供 sparse schema 推導。
 * 不符合即拋例外。
 * 需要安裝 `apache-arrow`（optional peer dependency）。
 */
export async function readArrow(text: string): Promise<ArrowTable> {
  const ipcBytes = wasmReadArrow(text);
  const { tableFromIPC } = await loadArrowIpc();
  return tableFromIPC(ipcBytes);
}

/**
 * 將 Arrow Table 序列化為 T-TOON Tabular 文字（async）
 *
 * 需要安裝 `apache-arrow`（optional peer dependency）。
 */
export async function stringifyArrow(table: ArrowTable, options?: SerializeOptions): Promise<string> {
  const { tableToIPC } = await loadArrowIpc();
  const ipcBytes = tableToIPC(table, 'stream');
  return wasmStringifyArrowTtoon(ipcBytes, options);
}

/**
 * 將 JavaScript 值序列化為 T-JSON 文字
 *
 * 輸出 JSON-like 的 {}/[] 括號格式，值層使用 typed 語法（uuid(...)、123.45m 等）。
 */
export function toTjson(value: ToonInput, options?: TjsonSerializeOptions): string {
  const irNode = jsToIr(value);
  return wasmStringifyTjson(irNode, options);
}

/**
 * 將 T-TOON 文字直接轉為 T-JSON 文字，不經過 JS object 中轉
 */
export function ttoonToTjson(text: string, options?: TtoonToTjsonOptions): string {
  const mode = options?.mode ?? 'compat';
  try {
    return wasmTtoonToTjson(text, mode, options);
  } catch (error) {
    throw transcodeError('ttoon_to_tjson', 'parse', error);
  }
}

/**
 * 將 Arrow Table 序列化為 T-JSON 文字（async）
 *
 * 輸出 list-of-objects 格式：`[{"col": val, ...}, ...]`
 * 需要安裝 `apache-arrow`（optional peer dependency）。
 */
export async function stringifyArrowTjson(table: ArrowTable, options?: TjsonSerializeOptions): Promise<string> {
  const { tableToIPC } = await loadArrowIpc();
  const ipcBytes = tableToIPC(table, 'stream');
  return wasmStringifyArrowTjson(ipcBytes, options);
}

/**
 * 偵測 T-TOON/T-JSON 文字的格式類型
 *
 * 回傳值：
 *   'ttoon' — T-TOON（縮排式 key-value 或 Tabular 表格）
 *   'tjson' — T-JSON（JSON-like { } / [ ]）
 */
export function detectFormat(text: string): Format {
  return detectFormatInternal(text);
}

/**
 * 全域注入 Codec 實作
 *
 * @example
 * await use({ decimal: codec })
 * await use({ date: dateCodec, decimal: decimalCodec })
 */
export async function use(codecs: CodecRegistry): Promise<void> {
  return useCodecs(codecs);
}

// ─── 低階 Arrow API（進階用途）────────────────────────────────────────────────

export type { ArrowTable };
