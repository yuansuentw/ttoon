/**
 * IrNode ↔ 原生 JavaScript 值的轉換
 *
 * JS 輸出規則（預設）：
 *   null     → null
 *   bool     → boolean
 *   int      → number
 *   float    → number
 *   decimal  → string（移除 'm' 後綴）；已註冊 Codec → 對應型別
 *   string   → string
 *   date     → string "YYYY-MM-DD"；已註冊 Codec → 對應型別
 *   time     → string "HH:MM:SS"；已註冊 Codec → 對應型別
 *   datetime → string ISO 8601；已註冊 Codec → 對應型別
 *   uuid     → string；已註冊 Codec → 對應型別
 *   binary   → Uint8Array；已註冊 Codec → 對應型別
 *   list     → Array
 *   object   → Record<string, unknown>
 *
 * JS 輸入規則（序列化時）：
 *   null                   → Null
 *   boolean                → Bool
 *   number (integer)       → Int
 *   number (non-integer)   → Float
 *   bigint                 → Int（若在 safe 範圍）
 *   string                 → String
 *   Uint8Array             → Binary
 *   Date                   → DateTime
 *   ToonTagged             → 對應的 typed node（uuid/decimal/date/time/datetime）
 *   Codec 型別             → Codec.toPayload()（若已註冊）
 *   plain object           → Object
 */
import { serializeError } from './errors.js';
import {
  IrNode,
  irNull, irBool, irInt, irInt64, isValidI64, irFloat, irDecimal, irStr,
  irDate, irTime, irDatetime, irUuid, irBinary,
  irList, irObject,
} from './ir.js';
import { ToonError } from './errors.js';
import type { Codec, CodecPayload, CodecRegistry, CodecType, IntPayload } from './codec.js';
import { getCodec, getCodecs } from './codec.js';
import { validateUuidContent } from './typed_parse.js';

// ─── 公開類型 ─────────────────────────────────────────────────────────────────

/** Tagged 型別，供序列化時標記特殊 T-TOON 型別 */
export type ToonTagged =
  | { readonly $type: 'uuid'; readonly value: string }
  | { readonly $type: 'decimal'; readonly value: string }
  | { readonly $type: 'date'; readonly value: string }
  | { readonly $type: 'time'; readonly value: string }
  | { readonly $type: 'datetime'; readonly value: string };

type ToonCustomValue = object;

export type ToonInput =
  | null
  | boolean
  | number
  | bigint
  | string
  | Uint8Array
  | Date
  | ToonTagged
  | ToonInput[]
  | { [key: string]: ToonInput }
  | ToonCustomValue;

export type ToonOutput =
  | null
  | boolean
  | number
  | string
  | Uint8Array
  | ToonOutput[]
  | { [key: string]: ToonOutput }
  | ToonCustomValue;

// 輔助建構函式（供使用者標記特殊型別）
export const toon = {
  uuid: (value: string): ToonTagged => ({ $type: 'uuid', value }),
  decimal: (value: string): ToonTagged => ({ $type: 'decimal', value }),
  date: (value: string): ToonTagged => ({ $type: 'date', value }),
  time: (value: string): ToonTagged => ({ $type: 'time', value }),
  datetime: (value: string): ToonTagged => ({ $type: 'datetime', value }),
} as const;

// ─── IrNode → JS 值（C03：整合 Codec 查詢）────────────────────────────────────

/**
 * 將 IrNode 轉換為原生 JavaScript 值
 *
 * 若對應 typed type 已透過 use() 註冊 Codec，使用 codec.fromPayload() 轉換。
 * 否則使用預設轉換（decimal → string、date → string 等）。
 *
 * @internal
 */
export function irToJs(
  node: IrNode,
  codecOverrides?: CodecRegistry,
): ToonOutput {
  const resolveCodec = (type: CodecType): Codec | undefined =>
    codecOverrides?.[type] ?? getCodec(type);

  switch (node.k) {
    case 'null':     return null;
    case 'bool':     return node.v;
    case 'int': {
      const codec = resolveCodec('int');
      if (codec) return codec.fromPayload(irNodeToPayload(node)) as ToonOutput;
      // 預設行為：等同 intNumber({ overflow: 'throw' })
      if (node.v64 !== undefined) {
        throw new ToonError(
          `Int64 value ${node.v64} outside JS safe integer range; ` +
          `use intBigInt() codec or intNumber({ overflow: 'nan' | 'lossy' })`,
          'parse',
        );
      }
      return node.v;
    }
    case 'float':    return node.v;
    case 'decimal': {
      const codec = resolveCodec('decimal');
      if (codec) return codec.fromPayload(irNodeToPayload(node)) as ToonOutput;
      // 預設：移除 'm' 後綴，回傳 string
      return node.v.endsWith('m') ? node.v.slice(0, -1) : node.v;
    }
    case 'string':   return node.v;
    case 'date': {
      const codec = resolveCodec('date');
      if (codec) return codec.fromPayload(irNodeToPayload(node)) as ToonOutput;
      return node.v;
    }
    case 'time': {
      const codec = resolveCodec('time');
      if (codec) return codec.fromPayload(irNodeToPayload(node)) as ToonOutput;
      return node.v;
    }
    case 'datetime': {
      const codec = resolveCodec('datetime');
      if (codec) return codec.fromPayload(irNodeToPayload(node)) as ToonOutput;
      return node.v;
    }
    case 'uuid': {
      const codec = resolveCodec('uuid');
      if (codec) return codec.fromPayload(irNodeToPayload(node)) as ToonOutput;
      return node.v;
    }
    case 'binary': {
      const codec = resolveCodec('binary');
      if (codec) return codec.fromPayload(irNodeToPayload(node)) as ToonOutput;
      return node.v;
    }
    case 'list':     return node.v.map(n => irToJs(n, codecOverrides));
    case 'object': {
      const obj: { [key: string]: ToonOutput } = {};
      for (const [k, v] of node.v) obj[k] = irToJs(v, codecOverrides);
      return obj;
    }
  }
}

// ─── JS 值 → IrNode（C04：整合 Codec 查詢）────────────────────────────────────

/**
 * 將原生 JavaScript 值轉換為 IrNode
 *
 * @internal
 */
export function jsToIr(value: ToonInput): IrNode {
  // null
  if (value === null) return irNull();

  // boolean（必須在 number 前）
  if (typeof value === 'boolean') return irBool(value);

  // number
  if (typeof value === 'number') {
    if (Number.isInteger(value) && Number.isSafeInteger(value)) return irInt(value);
    return irFloat(value);
  }

  // bigint
  if (typeof value === 'bigint') {
    if (value >= BigInt(Number.MIN_SAFE_INTEGER) && value <= BigInt(Number.MAX_SAFE_INTEGER)) {
      return irInt(Number(value));
    }
    if (!isValidI64(value)) throw serializeError(`bigint value ${value} exceeds signed i64 range`);
    return irInt64(value);
  }

  // string
  if (typeof value === 'string') return irStr(value);

  // Uint8Array（binary）
  if (value instanceof Uint8Array) return irBinary(value);

  // Date → datetime ISO string
  if (value instanceof Date) return irDatetime(value.toISOString());

  // C04: 已註冊 Codec 的型別（在 ToonTagged 前檢查）
  if (typeof value === 'object' && !('$type' in value) && !Array.isArray(value)) {
    for (const [type, codec] of getCodecs()) {
      if (codec.is(value)) return payloadToIrNode(type, codec.toPayload(value as never));
    }
  }

  // Tagged types
  if (typeof value === 'object' && '$type' in value) {
    const tagged = value as ToonTagged;
    switch (tagged.$type) {
      case 'uuid':
        validateUuidContent(tagged.value, serializeError);
        return irUuid(tagged.value);
      case 'decimal': {
        const dv = tagged.value;
        const body = dv.startsWith('+') || dv.startsWith('-') ? dv.slice(1) : dv;
        if (!/^[\d.]+$/.test(body)) throw serializeError(`invalid decimal: ${dv}`);
        // 儲存時加上 'm' 後綴（IR 格式）
        return irDecimal(dv.endsWith('m') ? dv : dv + 'm');
      }
      case 'date':
        if (!/^\d{4}-\d{2}-\d{2}$/.test(tagged.value)) throw serializeError(`invalid date: ${tagged.value}`);
        return irDate(tagged.value);
      case 'time': {
        const tv = tagged.value;
        if (tv.length < 8 || tv[2] !== ':' || tv[5] !== ':') throw serializeError(`invalid time: ${tv}`);
        return irTime(tv);
      }
      case 'datetime':
        return irDatetime(tagged.value);
    }
  }

  // Array → List
  if (Array.isArray(value)) {
    return irList((value as ToonInput[]).map(jsToIr));
  }

  // Plain object → Object
  if (typeof value === 'object') {
    const map = new Map<string, IrNode>();
    for (const [k, v] of Object.entries(value as Record<string, ToonInput>)) {
      map.set(k, jsToIr(v));
    }
    return irObject(map);
  }

  throw serializeError(`unsupported value type: ${typeof value}`);
}

function irNodeToPayload(node: IrNode): CodecPayload {
  switch (node.k) {
    case 'int':
      return node.v64 === undefined
        ? { value: node.v }
        : { value: node.v, value64: node.v64 };
    case 'decimal':
    case 'date':
    case 'time':
    case 'datetime':
    case 'uuid':
      return node.v;
    case 'binary':
      return node.v;
    default:
      throw new ToonError(`unsupported codec node kind: ${node.k}`, 'parse');
  }
}

function payloadToIrNode(type: CodecType, payload: CodecPayload): IrNode {
  switch (type) {
    case 'int': {
      const intPayload = expectIntPayload(payload);
      if (intPayload.value64 !== undefined) {
        if (!isValidI64(intPayload.value64)) {
          throw serializeError(`int codec payload.value64 ${intPayload.value64} exceeds signed i64 range`);
        }
        return irInt64(intPayload.value64);
      }
      if (!Number.isSafeInteger(intPayload.value)) {
        throw serializeError(`int codec payload.value ${intPayload.value} is not a safe integer`);
      }
      return irInt(intPayload.value);
    }
    case 'decimal':
      return irDecimal(expectStringPayload(type, payload));
    case 'date':
      return irDate(expectStringPayload(type, payload));
    case 'time':
      return irTime(expectStringPayload(type, payload));
    case 'datetime':
      return irDatetime(expectStringPayload(type, payload));
    case 'uuid':
      return irUuid(expectStringPayload(type, payload));
    case 'binary':
      return irBinary(expectBinaryPayload(payload));
  }
}

function expectIntPayload(payload: CodecPayload): IntPayload {
  if (typeof payload === 'string' || payload instanceof Uint8Array) {
    throw serializeError('int codec must return IntPayload');
  }
  return payload;
}

function expectStringPayload(type: Exclude<CodecType, 'int' | 'binary'>, payload: CodecPayload): string {
  if (typeof payload !== 'string') {
    throw serializeError(`${type} codec must return string payload`);
  }
  return payload;
}

function expectBinaryPayload(payload: CodecPayload): Uint8Array {
  if (!(payload instanceof Uint8Array)) {
    throw serializeError('binary codec must return Uint8Array payload');
  }
  return payload;
}
