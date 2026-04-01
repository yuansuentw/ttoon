/**
 * T-TOON Serializer 測試（公開 API 層級）
 *
 * 序列化走 WASM bridge；此檔案透過 stringify / parse 公開 API 驗證行為。
 * 共用 fixture 測試案例位於 fixture_serialize.test.ts。
 */
import { strict as assert } from 'node:assert';
import { describe, it } from 'node:test';
import { parse, stringify, toon } from '../index.js';

// ─── 基本型別序列化 ─────────────────────────────────────────────────────────

describe('stringify: basic types', () => {
  it('serializes null and bool', () => {
    const result = stringify({ active: true, deleted: false, value: null });
    assert.ok(result.includes('active: true'));
    assert.ok(result.includes('deleted: false'));
    assert.ok(result.includes('value: null'));
  });

  it('serializes decimal', () => {
    const result = stringify({ price: toon.decimal('99.99') });
    assert.ok(result.includes('price: 99.99m'));
  });

  it('serializes date / time / datetime', () => {
    const result = stringify({
      d: toon.date('2026-02-27'),
      t: toon.time('10:30:00'),
      dt: toon.datetime('2026-02-27T10:30:00Z'),
    });
    assert.ok(result.includes('d: 2026-02-27'));
    assert.ok(result.includes('t: 10:30:00'));
    assert.ok(result.includes('dt: 2026-02-27T10:30:00Z'));
  });

  it('strings requiring quotes should be quoted', () => {
    const result = stringify({ k: 'true' });
    assert.ok(result.includes('"true"'));
  });
});

// ─── Roundtrip ────────────────────────────────────────────────────────────────

describe('roundtrip (stringify → parse)', () => {
  it('null value roundtrip', () => {
    const original = { name: 'Alice', score: null };
    const text = stringify(original as Parameters<typeof stringify>[0]);
    const result = parse(text);
    assert.deepEqual(result, original);
  });

  it('UUID roundtrip', () => {
    const uuid = '550e8400-e29b-41d4-a716-446655440000';
    const original = { id: toon.uuid(uuid) };
    const text = stringify(original);
    assert.ok(text.includes(`uuid(${uuid})`));
    const result = parse(text) as Record<string, string>;
    assert.equal(result['id'], uuid);
  });

  it('decimal roundtrip', () => {
    const original = { price: toon.decimal('99.99') };
    const text = stringify(original);
    assert.ok(text.includes('99.99m'));
    const result = parse(text) as Record<string, string>;
    assert.equal(result['price'], '99.99');
  });

  it('date roundtrip', () => {
    const original = { d: toon.date('2026-02-27') };
    const text = stringify(original);
    const result = parse(text) as Record<string, string>;
    assert.equal(result['d'], '2026-02-27');
  });

  it('time roundtrip', () => {
    const original = { t: toon.time('10:30:00') };
    const text = stringify(original);
    const result = parse(text) as Record<string, string>;
    assert.equal(result['t'], '10:30:00');
  });

  it('tabular roundtrip', () => {
    const original = [
      { name: 'Alice', score: 95 },
      { name: 'Bob', score: 87 },
    ];
    const text = stringify(original);
    assert.ok(text.startsWith('[2]{name,score}:'));
    const result = parse(text);
    assert.deepEqual(result, original);
  });

  it('nested object roundtrip', () => {
    const original = { user: { name: 'Carol', age: 28 } };
    const text = stringify(original);
    const result = parse(text);
    assert.deepEqual(result, original);
  });

  it('invalid UUID tag should be rejected before serialization', () => {
    assert.throws(
      () => stringify({ id: toon.uuid('426AC144-7477-4A90-93DE-33F879C62D4D') }),
      { message: /invalid uuid: must be lowercase hex/ },
    );
  });
});
