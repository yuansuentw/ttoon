/**
 * v004: Int64 無損 IR 儲存與 int Codec 測試
 *
 * 涵蓋：
 *   - 低階 Parser / IR 測試（parseUnit, parseNumberLike）
 *   - Public parse() API 整合測試（irToJs + Codec）
 *   - Serialize roundtrip 測試（透過公開 API）
 *   - jsToIr bigint 方向測試
 *   - Arrow roundtrip 測試
 */
import { strict as assert } from 'node:assert';
import { describe, it, afterEach } from 'node:test';
import { parseUnit, parseNumberLike } from '../typed_parse.js';
import { irToJs, jsToIr } from '../convert.js';
import { irInt, irInt64, isValidI64 } from '../ir.js';
import type { IrNode } from '../ir.js';
import { parse, stringify, toTjson, use } from '../index.js';
import { intNumber, intBigInt } from '../int_codec.js';
import { _resetCodecRegistry, type IntPayload } from '../codec.js';

afterEach(() => {
  _resetCodecRegistry();
});

// ─── Helpers ──────────────────────────────────────────────────────────────────

function assertIntNode(node: IrNode, expectedV: number, expectedV64?: bigint): void {
  assert.equal(node.k, 'int', `expected int node, got ${node.k}`);
  const n = node as { k: 'int'; v: number; v64?: bigint };
  if (expectedV64 !== undefined) {
    assert.ok(Number.isNaN(n.v), `expected v=NaN for overflow int, got ${n.v}`);
    assert.equal(n.v64, expectedV64);
  } else {
    assert.equal(n.v, expectedV);
    assert.equal(n.v64, undefined, 'v64 should be undefined for safe integers');
  }
}

function assertIntPayload(payload: IntPayload, expectedV: number, expectedV64?: bigint): void {
  if (expectedV64 !== undefined) {
    assert.ok(Number.isNaN(payload.value), `expected value=NaN for overflow int payload, got ${payload.value}`);
    assert.equal(payload.value64, expectedV64);
  } else {
    assert.equal(payload.value, expectedV);
    assert.equal(payload.value64, undefined, 'value64 should be undefined for safe integers');
  }
}

// ─── isValidI64 ──────────────────────────────────────────────────────────────

describe('isValidI64', () => {
  it('accepts i64 min/max', () => {
    assert.ok(isValidI64(-(2n ** 63n)));
    assert.ok(isValidI64(2n ** 63n - 1n));
  });

  it('rejects values outside i64 range', () => {
    assert.ok(!isValidI64(2n ** 63n));
    assert.ok(!isValidI64(-(2n ** 63n) - 1n));
  });

  it('accepts zero and small values', () => {
    assert.ok(isValidI64(0n));
    assert.ok(isValidI64(42n));
    assert.ok(isValidI64(-42n));
  });
});

// ─── 低階 Parser / IR 測試 ──────────────────────────────────────────────────

describe('Low-level Parser: parseUnit — Int64 overflow', () => {
  it('safe integer produces normal irInt', () => {
    assertIntNode(parseUnit('42'), 42);
  });

  it('MAX_SAFE_INTEGER is still safe', () => {
    assertIntNode(parseUnit('9007199254740991'), 9007199254740991);
  });

  it('MAX_SAFE_INTEGER + 1 produces irInt64', () => {
    assertIntNode(parseUnit('9007199254740992'), NaN, 9007199254740992n);
  });

  it('2^53 + 1 produces irInt64', () => {
    assertIntNode(parseUnit('9007199254740993'), NaN, 9007199254740993n);
  });

  it('negative overflow -9007199254740993 produces irInt64', () => {
    assertIntNode(parseUnit('-9007199254740993'), NaN, -9007199254740993n);
  });

  it('i64 MAX produces irInt64', () => {
    assertIntNode(parseUnit('9223372036854775807'), NaN, 9223372036854775807n);
  });

  it('i64 MIN produces irInt64', () => {
    assertIntNode(parseUnit('-9223372036854775808'), NaN, -9223372036854775808n);
  });

  it('exceeding i64 range throws parseError', () => {
    assert.throws(() => parseUnit('9223372036854775808'), /exceeds signed i64 range/);
  });

  it('far exceeding i64 range throws parseError', () => {
    assert.throws(() => parseUnit('99999999999999999999999999999999999999'), /exceeds signed i64 range/);
  });
});

describe('Low-level Parser: parseNumberLike — Int64 overflow', () => {
  it('safe integer parses normally', () => {
    assertIntNode(parseNumberLike('42'), 42);
  });

  it('MAX_SAFE_INTEGER + 1 produces irInt64', () => {
    assertIntNode(parseNumberLike('9007199254740992'), NaN, 9007199254740992n);
  });

  it('exceeding i64 range throws parseError', () => {
    assert.throws(() => parseNumberLike('9223372036854775808'), /exceeds signed i64 range/);
  });
});

// ─── 序列化溢出 int（透過公開 API）────────────────────────────────────────────

describe('Serialization: int64 via public API', () => {
  it('T-JSON serializes overflow int', () => {
    assert.equal(toTjson(9007199254740993n), '9007199254740993');
  });

  it('T-TOON serializes overflow int', () => {
    assert.equal(stringify(9007199254740993n).trim(), '9007199254740993');
  });

  it('safe int serialization unaffected', () => {
    assert.equal(toTjson(42), '42');
  });

  it('i64 MAX serialization', () => {
    assert.equal(toTjson(9223372036854775807n), '9223372036854775807');
  });

  it('i64 MIN serialization', () => {
    assert.equal(toTjson(-9223372036854775808n), '-9223372036854775808');
  });
});

// ─── 文字 Roundtrip 測試 ────────────────────────────────────────────────────

describe('Roundtrip: serialize → parse', () => {
  it('overflow int roundtrip (T-JSON)', () => {
    const text = toTjson(9007199254740993n);
    const reparsed = parseUnit(text);
    assertIntNode(reparsed, NaN, 9007199254740993n);
  });

  it('i64 MAX roundtrip (T-JSON)', () => {
    const text = toTjson(9223372036854775807n);
    const reparsed = parseUnit(text);
    assertIntNode(reparsed, NaN, 9223372036854775807n);
  });

  it('i64 MIN roundtrip (T-JSON)', () => {
    const text = toTjson(-9223372036854775808n);
    const reparsed = parseUnit(text);
    assertIntNode(reparsed, NaN, -9223372036854775808n);
  });

  it('safe int roundtrip unaffected', () => {
    const text = toTjson(42);
    const reparsed = parseUnit(text);
    assertIntNode(reparsed, 42);
  });
});

// ─── jsToIr bigint 方向 ────────────────────────────────────────────────────

describe('jsToIr: bigint direction', () => {
  it('safe range bigint → irInt(number)', () => {
    const node = jsToIr(42n);
    assertIntNode(node, 42);
  });

  it('overflow bigint → irInt64', () => {
    const node = jsToIr(9007199254740993n);
    assertIntNode(node, NaN, 9007199254740993n);
  });

  it('i64 MAX bigint → irInt64', () => {
    const node = jsToIr(9223372036854775807n);
    assertIntNode(node, NaN, 9223372036854775807n);
  });

  it('exceeding i64 range throws serializeError', () => {
    assert.throws(() => jsToIr(9223372036854775808n), /exceeds signed i64 range/);
  });

  it('negative exceeding i64 range throws serializeError', () => {
    assert.throws(() => jsToIr(-(2n ** 63n) - 1n), /exceeds signed i64 range/);
  });
});

// ─── irToJs 預設行為 ────────────────────────────────────────────────────────

describe('irToJs: int default behavior (no Codec)', () => {
  it('safe int returns number normally', () => {
    assert.equal(irToJs(irInt(42)), 42);
  });

  it('overflow int throws by default', () => {
    assert.throws(
      () => irToJs(irInt64(9007199254740993n)),
      /outside JS safe integer range/,
    );
  });
});

// ─── Public parse() API + Codec 整合 ────────────────────────────────────────

describe('Public parse() API + int Codec integration', () => {
  it('default (no Codec): overflow int throws', () => {
    assert.throws(
      () => parse('9007199254740993'),
      /outside JS safe integer range/,
    );
  });

  it('intNumber({ overflow: "throw" }): overflow throws', async () => {
    await use({ int: intNumber({ overflow: 'throw' }) });
    assert.throws(
      () => parse('9007199254740993'),
      /outside JS safe integer range/,
    );
  });

  it('intNumber({ overflow: "nan" }): overflow returns NaN', async () => {
    await use({ int: intNumber({ overflow: 'nan' }) });
    const result = parse('9007199254740993');
    assert.ok(Number.isNaN(result as number));
  });

  it('intNumber({ overflow: "lossy" }): overflow returns Number(v64)', async () => {
    await use({ int: intNumber({ overflow: 'lossy' }) });
    const result = parse('9007199254740993');
    assert.equal(typeof result, 'number');
    // Number(9007199254740993n) === 9007199254740992（最接近的 double）
    assert.equal(result, 9007199254740992);
  });

  it('intBigInt(): safe int returns bigint', async () => {
    await use({ int: intBigInt() });
    const result = parse('42');
    assert.equal(result, 42n);
  });

  it('intBigInt(): overflow int returns bigint', async () => {
    await use({ int: intBigInt() });
    const result = parse('9007199254740993');
    assert.equal(result, 9007199254740993n);
  });

  it('intBigInt(): i64 MAX returns bigint', async () => {
    await use({ int: intBigInt() });
    const result = parse('9223372036854775807');
    assert.equal(result, 9223372036854775807n);
  });

  it('per-call codec overrides global', async () => {
    await use({ int: intNumber({ overflow: 'throw' }) });
    // per-call 使用 intBigInt 覆寫
    const result = parse('9007199254740993', { codecs: { int: intBigInt() } });
    assert.equal(result, 9007199254740993n);
  });
});

// ─── intNumber codec 直接測試 ───────────────────────────────────────────────

describe('intNumber codec', () => {
  it('fromPayload: safe int returns number', () => {
    const codec = intNumber();
    assert.equal(codec.fromPayload({ value: 42 }), 42);
  });

  it('fromPayload: overflow throw', () => {
    const codec = intNumber();
    assert.throws(() => codec.fromPayload({ value: NaN, value64: 9007199254740993n }), /outside JS safe integer range/);
  });

  it('fromPayload: overflow nan', () => {
    const codec = intNumber({ overflow: 'nan' });
    assert.ok(Number.isNaN(codec.fromPayload({ value: NaN, value64: 9007199254740993n })));
  });

  it('fromPayload: overflow lossy', () => {
    const codec = intNumber({ overflow: 'lossy' });
    assert.equal(codec.fromPayload({ value: NaN, value64: 9007199254740993n }), 9007199254740992);
  });

  it('toPayload: safe integer', () => {
    const codec = intNumber();
    assertIntPayload(codec.toPayload(42) as IntPayload, 42);
  });

  it('toPayload: non-safe integer throws', () => {
    const codec = intNumber();
    assert.throws(() => codec.toPayload(NaN), /not a safe integer/);
    assert.throws(() => codec.toPayload(1.5), /not a safe integer/);
  });

  it('is: identifies safe integers', () => {
    const codec = intNumber();
    assert.ok(codec.is(42));
    assert.ok(codec.is(0));
    assert.ok(!codec.is(1.5));
    assert.ok(!codec.is(NaN));
    assert.ok(!codec.is('42'));
    assert.ok(!codec.is(42n));
  });
});

// ─── intBigInt codec 直接測試 ───────────────────────────────────────────────

describe('intBigInt codec', () => {
  it('fromPayload: safe int returns bigint', () => {
    const codec = intBigInt();
    assert.equal(codec.fromPayload({ value: 42 }), 42n);
  });

  it('fromPayload: overflow int returns bigint', () => {
    const codec = intBigInt();
    assert.equal(codec.fromPayload({ value: NaN, value64: 9007199254740993n }), 9007199254740993n);
  });

  it('fromPayload: i64 MAX returns bigint', () => {
    const codec = intBigInt();
    assert.equal(codec.fromPayload({ value: NaN, value64: 9223372036854775807n }), 9223372036854775807n);
  });

  it('toPayload: safe range bigint', () => {
    const codec = intBigInt();
    assertIntPayload(codec.toPayload(42n) as IntPayload, 42);
  });

  it('toPayload: overflow bigint', () => {
    const codec = intBigInt();
    assertIntPayload(codec.toPayload(9007199254740993n) as IntPayload, NaN, 9007199254740993n);
  });

  it('toPayload: exceeds i64 throws', () => {
    const codec = intBigInt();
    assert.throws(() => codec.toPayload(2n ** 63n), /exceeds signed i64 range/);
  });

  it('is: identifies bigint', () => {
    const codec = intBigInt();
    assert.ok(codec.is(42n));
    assert.ok(codec.is(0n));
    assert.ok(!codec.is(42));
    assert.ok(!codec.is('42'));
  });
});
