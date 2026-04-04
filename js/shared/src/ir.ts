/**
 * 內部 IR（中間表示）— 對應 Rust ttoon_core::ir::Node
 *
 * 使用 discriminated union，k 為 kind 的縮寫。
 * 整數以 number 儲存（safe integer 範圍）；超出範圍請用 bigint 選項。
 */
export type IrNode =
  | { readonly k: 'null' }
  | { readonly k: 'bool'; readonly v: boolean }
  | { readonly k: 'int'; readonly v: number; readonly v64?: bigint }  // safe integer; v64 for overflow
  | { readonly k: 'float'; readonly v: number }
  | { readonly k: 'decimal'; readonly v: string }    // 帶 'm' 後綴，e.g. "3.14m"
  | { readonly k: 'string'; readonly v: string }
  | { readonly k: 'date'; readonly v: string }       // YYYY-MM-DD
  | { readonly k: 'time'; readonly v: string }       // HH:MM:SS[.fff]
  | { readonly k: 'datetime'; readonly v: string }   // ISO 8601
  | { readonly k: 'uuid'; readonly v: string }       // 36 字元 UUID 字串
  | { readonly k: 'binary'; readonly v: Uint8Array }
  | { readonly k: 'list'; readonly v: IrNode[] }
  | { readonly k: 'object'; readonly v: Map<string, IrNode> };

// ─── 建構函式縮寫 ─────────────────────────────────────────────────────────────

export function irNull(): IrNode { return { k: 'null' }; }
export function irBool(v: boolean): IrNode { return { k: 'bool', v }; }
export function irInt(v: number): IrNode { return { k: 'int', v }; }
export function irFloat(v: number): IrNode { return { k: 'float', v }; }
export function irDecimal(v: string): IrNode { return { k: 'decimal', v }; }
export function irStr(v: string): IrNode { return { k: 'string', v }; }
export function irDate(v: string): IrNode { return { k: 'date', v }; }
export function irTime(v: string): IrNode { return { k: 'time', v }; }
export function irDatetime(v: string): IrNode { return { k: 'datetime', v }; }
export function irUuid(v: string): IrNode { return { k: 'uuid', v }; }
export function irBinary(v: Uint8Array): IrNode { return { k: 'binary', v }; }
export function irList(v: IrNode[]): IrNode { return { k: 'list', v }; }
export function irObject(v: Map<string, IrNode>): IrNode { return { k: 'object', v }; }

// ─── Int64 支援 ──────────────────────────────────────────────────────────────

const MIN_I64 = -(2n ** 63n);
const MAX_I64 = 2n ** 63n - 1n;

/** i64 邊界檢查（純 predicate，不拋錯） */
export function isValidI64(v: bigint): boolean {
  return v >= MIN_I64 && v <= MAX_I64;
}

/** 建構溢出 int 節點（純建構函式，不含邊界檢查） */
export function irInt64(v: bigint): IrNode {
  return { k: 'int', v: NaN, v64: v };
}
