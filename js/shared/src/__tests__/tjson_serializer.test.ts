/**
 * T-JSON 序列化器測試
 */
import { strict as assert } from 'node:assert';
import { describe, it, beforeEach, afterEach } from 'node:test';
import { toTjson, parse, toon, isArrowTable } from '../index.js';
import { use, _resetCodecRegistry, type Codec, type CodecPayload } from '../codec.js';

function expectStringPayload(payload: CodecPayload, label: string): string {
  if (typeof payload !== 'string') throw new Error(`not ${label}`);
  return payload;
}

describe('toTjson', () => {
  it('serializes basic object', () => {
    const result = toTjson({ name: 'Alice', age: 30, active: true });
    assert.ok(result.startsWith('{'));
    assert.ok(result.includes('"name": "Alice"'));
    assert.ok(result.includes('"age": 30'));
    assert.ok(result.includes('"active": true'));
  });

  it('serializes null', () => {
    const result = toTjson({ x: null });
    assert.ok(result.includes('"x": null'));
  });

  it('serializes list', () => {
    const result = toTjson([1, 2, 3]);
    assert.strictEqual(result, '[1, 2, 3]');
  });

  it('serializes list of objects (outputs list-of-objects, not tabular)', () => {
    const result = toTjson([{ a: 1 }, { a: 2 }]);
    assert.ok(result.startsWith('['));
    assert.ok(result.includes('"a": 1'));
    assert.ok(result.includes('"a": 2'));
  });

  it('serializes uuid', () => {
    const uuid = '550e8400-e29b-41d4-a716-446655440000';
    const result = toTjson({ id: toon.uuid(uuid) });
    assert.ok(result.includes(`uuid(${uuid})`));
  });

  it('serializes decimal', () => {
    const result = toTjson({ price: toon.decimal('99.99') });
    assert.ok(result.includes('99.99m'));
  });

  it('serializes date / time / datetime', () => {
    const result = toTjson({
      d: toon.date('2026-02-27'),
      t: toon.time('10:30:00'),
      dt: toon.datetime('2026-02-27T10:30:00Z'),
    });
    assert.ok(result.includes('2026-02-27'));
    assert.ok(result.includes('10:30:00'));
    assert.ok(result.includes('2026-02-27T10:30:00Z'));
  });

  it('empty binary roundtrips with binaryFormat=b64', () => {
    const text = toTjson({ payload: new Uint8Array([]) }, { binaryFormat: 'b64' });
    assert.equal(text, '{"payload": b64()}');
    const result = parse(text) as Record<string, Uint8Array>;
    assert.ok(result['payload'] instanceof Uint8Array);
    assert.equal(result['payload'].length, 0);
  });

  it('roundtrip: toTjson → parse', () => {
    const original = { name: 'Bob', score: 87, active: false };
    const text = toTjson(original);
    const result = parse(text);
    assert.deepEqual(result, original);
  });

  it('roundtrip: nested object', () => {
    const original = { user: { name: 'Carol', age: 28 } };
    const text = toTjson(original);
    const result = parse(text);
    assert.deepEqual(result, original);
  });

  it('roundtrip: with uuid and decimal', () => {
    const uuid = '550e8400-e29b-41d4-a716-446655440000';
    const original = { id: toon.uuid(uuid), price: toon.decimal('12.50') };
    const text = toTjson(original);
    const result = parse(text) as Record<string, string>;
    assert.equal(result['id'], uuid);
    assert.equal(result['price'], '12.50');
  });

  it('canonical WASM parser rejects lone surrogate', () => {
    assert.throws(() => parse('{"s": "\\uD83D"}'));
  });
});

// ─── toTjson() codec 自訂類別測試（P2 補齊）──────────────────────────────────

class Price {
  constructor(public raw: string) {}
}

describe('toTjson() with custom codec', () => {
  beforeEach(() => { _resetCodecRegistry(); });
  afterEach(() => { _resetCodecRegistry(); });

  it('custom decimal codec serializes correctly via toTjson()', async () => {
    await use({ decimal: {
      type: 'decimal',
      is(v: unknown): v is Price { return v instanceof Price; },
      toPayload(v: Price): string { return v.raw + 'm'; },
      fromPayload(payload: CodecPayload): Price {
        const raw = expectStringPayload(payload, 'decimal');
        const s = raw.endsWith('m') ? raw.slice(0, -1) : raw;
        return new Price(s);
      },
    } satisfies Codec<Price> });

    const result = toTjson({ price: new Price('42.50') });
    assert.ok(result.includes('42.50m'), `expected decimal in toTjson output, got: ${result}`);
  });

  it('toTjson() → parse() roundtrip with custom codec', async () => {
    await use({ decimal: {
      type: 'decimal',
      is(v: unknown): v is Price { return v instanceof Price; },
      toPayload(v: Price): string { return v.raw + 'm'; },
      fromPayload(payload: CodecPayload): Price {
        const raw = expectStringPayload(payload, 'decimal');
        const s = raw.endsWith('m') ? raw.slice(0, -1) : raw;
        return new Price(s);
      },
    } satisfies Codec<Price> });

    const text = toTjson({ price: new Price('99.99') });
    const parsed = parse(text) as Record<string, Price>;
    assert.ok(parsed['price'] instanceof Price, 'roundtrip should produce Price via codec fromPayload');
    assert.equal(parsed['price'].raw, '99.99');
  });
});

// ─── toTjson() Arrow 誤用負向測試（P2 補齊）──────────────────────────────────

describe('toTjson() rejects Arrow-like input', () => {
  it('passing Arrow-like object should throw, not silently fail', () => {
    const fakeArrowTable = {
      schema: { fields: [] },
      numRows: 0,
      getChild: () => null,
    };
    assert.ok(isArrowTable(fakeArrowTable), 'precondition: should be detected as Arrow table');
    assert.throws(() => {
      toTjson(fakeArrowTable as never);
    });
  });
});
