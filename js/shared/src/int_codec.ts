/**
 * 內建 int Codec — intNumber / intBigInt
 *
 * intNumber：回傳 number，溢出時依 overflow 選項處理（throw / nan / lossy）
 * intBigInt：一律回傳 bigint，涵蓋完整 i64 範圍
 */
import type { Codec, IntPayload } from './codec.js';
import { isValidI64 } from './ir.js';
import { ToonError } from './errors.js';

// ─── intNumber ───────────────────────────────────────────────────────────────

/**
 * int Codec：回傳 number
 *
 * @param opts.overflow 溢出策略（預設 'throw'）
 *   - 'throw'：拋出 ToonError
 *   - 'nan'：回傳 NaN
 *   - 'lossy'：回傳 Number(v64)（最接近的 double）
 *
 * @remarks
 * `toPayload` 和 `is` 僅在處理使用者自訂 class wrapper 時觸發。
 * 對於 primitive `number`，`jsToIr` 內部以 `typeof` 快速路徑直接攔截，不經過 Codec。
 */
export function intNumber(opts?: {
  overflow?: 'throw' | 'nan' | 'lossy';
}): Codec<number> {
  const overflow = opts?.overflow ?? 'throw';
  return {
    type: 'int',
    fromPayload(payload): number {
      const intPayload = payload as IntPayload;
      if (intPayload.value64 === undefined) return intPayload.value;
      switch (overflow) {
        case 'throw':
          throw new ToonError(
            `Int64 value ${intPayload.value64} outside JS safe integer range; ` +
            `use intBigInt() codec or intNumber({ overflow: 'nan' | 'lossy' })`,
            'parse',
          );
        case 'nan':   return NaN;
        case 'lossy': return Number(intPayload.value64);
      }
    },
    toPayload(value: number): IntPayload {
      if (!Number.isSafeInteger(value)) {
        throw new ToonError(`intNumber: value ${value} is not a safe integer`, 'serialize');
      }
      return { value };
    },
    is(value: unknown): value is number {
      return typeof value === 'number' && Number.isSafeInteger(value);
    },
  };
}

// ─── intBigInt ───────────────────────────────────────────────────────────────

/**
 * int Codec：一律回傳 bigint（涵蓋完整 i64 範圍）
 *
 * @remarks
 * `toPayload` 和 `is` 僅在處理使用者自訂 class wrapper 時觸發。
 * 對於 primitive `bigint`，`jsToIr` 內部以 `typeof` 快速路徑直接攔截，不經過 Codec。
 */
export function intBigInt(): Codec<bigint> {
  return {
    type: 'int',
    fromPayload(payload): bigint {
      const intPayload = payload as IntPayload;
      return intPayload.value64 ?? BigInt(intPayload.value);
    },
    toPayload(value: bigint): IntPayload {
      if (!isValidI64(value)) {
        throw new ToonError(`intBigInt: bigint value ${value} exceeds signed i64 range`, 'serialize');
      }
      if (value >= BigInt(Number.MIN_SAFE_INTEGER) && value <= BigInt(Number.MAX_SAFE_INTEGER)) {
        return { value: Number(value) };
      }
      return { value: NaN, value64: value };
    },
    is(value: unknown): value is bigint {
      return typeof value === 'bigint';
    },
  };
}
