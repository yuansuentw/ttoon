/**
 * JSON IR wire format conversion: WireNode ↔ IrNode.
 *
 * Used for transferring IR data across the WASM boundary.
 * Wire format matches the Rust WireNode serde output:
 *   {"k":"int","v":42}          — safe integer
 *   {"k":"int","v64":"999..."}  — i64 overflow
 *   {"k":"float","v":3.14}     — normal float
 *   {"k":"float","s":"nan"}    — special float
 *   {"k":"object","v":[{"k":"name","v":{...}}, ...]} — ordered KV pairs
 */
import type { IrNode } from './ir.js';

// ─── Wire types (JSON-parsed shapes) ────────────────────────────────────────

interface WireKV {
  k: string;
  v: WireRaw;
}

type WireRaw =
  | null
  | { k: 'null' }
  | { k: 'bool'; v: boolean }
  | { k: 'int'; v?: number; v64?: string }
  | { k: 'float'; v?: number; s?: string }
  | { k: 'decimal'; v: string }
  | { k: 'string'; v: string }
  | { k: 'date'; v: string }
  | { k: 'time'; v: string }
  | { k: 'datetime'; v: string }
  | { k: 'uuid'; v: string }
  | { k: 'binary'; v: string }
  | { k: 'list'; v: WireRaw[] }
  | { k: 'object'; v: WireKV[] };

// ─── Wire → IrNode ──────────────────────────────────────────────────────────

export function wireToIr(raw: WireRaw): IrNode {
  if (raw === null) return { k: 'null' };

  switch (raw.k) {
    case 'null':
      return { k: 'null' };
    case 'bool':
      return { k: 'bool', v: raw.v };
    case 'int': {
      if (raw.v64 !== undefined) {
        // i64 overflow: v=NaN marker, v64=bigint
        return { k: 'int', v: NaN, v64: BigInt(raw.v64) } as IrNode & { v64: bigint };
      }
      return { k: 'int', v: raw.v! };
    }
    case 'float': {
      if (raw.s !== undefined) {
        switch (raw.s) {
          case 'nan': return { k: 'float', v: NaN };
          case 'inf': return { k: 'float', v: Infinity };
          case '-inf': return { k: 'float', v: -Infinity };
          default: throw new Error(`unknown float special: ${raw.s}`);
        }
      }
      return { k: 'float', v: raw.v! };
    }
    case 'decimal':
      return { k: 'decimal', v: raw.v };
    case 'string':
      return { k: 'string', v: raw.v };
    case 'date':
      return { k: 'date', v: raw.v };
    case 'time':
      return { k: 'time', v: raw.v };
    case 'datetime':
      return { k: 'datetime', v: raw.v };
    case 'uuid':
      return { k: 'uuid', v: raw.v };
    case 'binary': {
      // Wire format: hex-encoded string → Uint8Array
      const hex = raw.v;
      const bytes = new Uint8Array(hex.length / 2);
      for (let i = 0; i < hex.length; i += 2) {
        bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16);
      }
      return { k: 'binary', v: bytes };
    }
    case 'list':
      return { k: 'list', v: raw.v.map(wireToIr) };
    case 'object': {
      const m = new Map<string, IrNode>();
      for (const kv of raw.v) {
        m.set(kv.k, wireToIr(kv.v));
      }
      return { k: 'object', v: m };
    }
    default:
      throw new Error(`unknown wire node kind: ${(raw as { k: string }).k}`);
  }
}

// ─── IrNode → Wire ──────────────────────────────────────────────────────────

export function irToWire(ir: IrNode): WireRaw {
  switch (ir.k) {
    case 'null':
      return { k: 'null' };
    case 'bool':
      return { k: 'bool', v: ir.v };
    case 'int': {
      const n = ir as { k: 'int'; v: number; v64?: bigint };
      if (n.v64 !== undefined) {
        return { k: 'int', v64: n.v64.toString() };
      }
      return { k: 'int', v: ir.v };
    }
    case 'float': {
      if (Number.isNaN(ir.v)) return { k: 'float', s: 'nan' };
      if (ir.v === Infinity) return { k: 'float', s: 'inf' };
      if (ir.v === -Infinity) return { k: 'float', s: '-inf' };
      return { k: 'float', v: ir.v };
    }
    case 'decimal':
      return { k: 'decimal', v: ir.v };
    case 'string':
      return { k: 'string', v: ir.v };
    case 'date':
      return { k: 'date', v: ir.v };
    case 'time':
      return { k: 'time', v: ir.v };
    case 'datetime':
      return { k: 'datetime', v: ir.v };
    case 'uuid':
      return { k: 'uuid', v: ir.v };
    case 'binary': {
      // Uint8Array → hex string
      let hex = '';
      for (const b of ir.v) {
        hex += b.toString(16).padStart(2, '0');
      }
      return { k: 'binary', v: hex };
    }
    case 'list':
      return { k: 'list', v: ir.v.map(irToWire) };
    case 'object': {
      const kvs: WireKV[] = [];
      for (const [k, v] of ir.v) {
        kvs.push({ k, v: irToWire(v) });
      }
      return { k: 'object', v: kvs };
    }
  }
}
