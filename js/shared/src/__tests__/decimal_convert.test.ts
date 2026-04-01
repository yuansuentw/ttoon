/**
 * Decimal 轉換測試（J12 後：Codec 系統）
 *
 * J12 移除了 prepareDecimalMode / _resetDecimalCache / parseAsync 及
 * Big / Decimal / BigNum 模式。Decimal 現在透過 Codec 系統處理。
 */
import { strict as assert } from 'node:assert';
import { describe, it, beforeEach } from 'node:test';
import { parse, stringify } from '../index.js';
import { irToJs } from '../convert.js';
import { irDate, irTime, irDatetime, irDecimal, irObject, irList } from '../ir.js';
import { use, _resetCodecRegistry, type Codec, type CodecPayload } from '../codec.js';
import { afterEach } from 'node:test';

function expectStringPayload(payload: CodecPayload, label: string): string {
  if (typeof payload !== 'string') throw new Error(`not ${label}`);
  return payload;
}

function createDecimalNumberCodec(): Codec<number> {
  return {
    type: 'decimal',
    fromPayload(payload: CodecPayload): number {
      const raw = expectStringPayload(payload, 'decimal');
      const s = raw.endsWith('m') ? raw.slice(0, -1) : raw;
      return Number(s);
    },
    toPayload(value: number): string {
      return String(value) + 'm';
    },
    is(value: unknown): value is number {
      return typeof value === 'number';
    },
  };
}

// ─── 預設行為（decimal → string，移除 'm' 後綴）──────────────────────────────

describe('Decimal default behavior', () => {
  it('default returns string (without m suffix)', () => {
    const result = parse('price: 99.99m\n');
    assert.deepEqual(result, { price: '99.99' });
  });

  it('negative decimal default returns string', () => {
    const result = parse('loss: -42.5m\n');
    assert.deepEqual(result, { loss: '-42.5' });
  });

  it('irToJs decimal default returns string', () => {
    const node = irDecimal('99.99m');
    const result = irToJs(node);
    assert.equal(result, '99.99');
  });
});

// ─── 巢狀結構中的 decimal ─────────────────────────────────────────────────────

describe('Decimal in nested structures', () => {
  it('irToJs handles decimal inside list', () => {
    const node = irList([irDecimal('1.5m'), irDecimal('2.5m')]);
    const result = irToJs(node) as string[];
    assert.equal(result[0], '1.5');
    assert.equal(result[1], '2.5');
  });

  it('irToJs handles decimal in nested list inside object', () => {
    const node = irObject(new Map([
      ['prices', irList([irDecimal('10.5m'), irDecimal('20.3m')])],
    ]));
    const result = irToJs(node) as Record<string, unknown>;
    assert.deepEqual(result.prices, ['10.5', '20.3']);
  });

  it('parse handles decimal in nested object', () => {
    const input = `order:\n  total: 99.99m\n  tax: 7.5m\n`;
    const result = parse(input) as Record<string, unknown>;
    const order = result.order as Record<string, unknown>;
    assert.equal(order.total, '99.99');
    assert.equal(order.tax, '7.5');
  });
});

// ─── Codec 系統（C01–C04）────────────────────────────────────────────────────

describe('Codec system', () => {
  beforeEach(async () => {
    _resetCodecRegistry();
  });
  afterEach(() => {
    _resetCodecRegistry();
  });

  it('without Codec, decimal returns string (default)', () => {
    const node = irDecimal('99.99m');
    assert.equal(irToJs(node), '99.99');
  });

  it('registered Codec uses fromPayload', async () => {
    await use({ decimal: createDecimalNumberCodec() });

    const node = irDecimal('99.99m');
    const result = irToJs(node) as number;
    assert.equal(result, 99.99);
  });

  it('Codec handles parsed decimal', async () => {
    await use({ decimal: createDecimalNumberCodec() });

    const result = parse('price: 42.5m\n') as Record<string, unknown>;
    assert.equal(result.price, 42.5);
  });

  it('parse supports per-call codecs override (no use needed)', () => {
    const result = parse('price: 42.5m\n', {
      codecs: { decimal: createDecimalNumberCodec() },
    }) as Record<string, unknown>;
    assert.equal(result.price, 42.5);

    const defaultResult = parse('price: 42.5m\n') as Record<string, unknown>;
    assert.equal(defaultResult.price, '42.5');
  });

  it('per-call codecs take precedence over global use()', async () => {
    await use({ decimal: {
      type: 'decimal',
      fromPayload(payload: CodecPayload): string {
        const raw = expectStringPayload(payload, 'decimal');
        const s = raw.endsWith('m') ? raw.slice(0, -1) : raw;
        return `global:${s}`;
      },
      toPayload(value: string): string {
        const raw = value.startsWith('global:') ? value.slice('global:'.length) : value;
        return raw.endsWith('m') ? raw : raw + 'm';
      },
      is(value: unknown): value is string {
        return typeof value === 'string' && value.startsWith('global:');
      },
    } });

    const perCall = parse('price: 42.5m\n', {
      codecs: { decimal: createDecimalNumberCodec() },
    }) as Record<string, unknown>;
    assert.equal(perCall.price, 42.5);

    const globalResult = parse('price: 42.5m\n') as Record<string, unknown>;
    assert.equal(globalResult.price, 'global:42.5');
  });
});

// ─── stringify() codec is + toPayload 路徑（C04）───────────────────────────────

class MyDecimal {
  constructor(public raw: string) {}
}

describe('stringify() codec is + toPayload', () => {
  beforeEach(() => { _resetCodecRegistry(); });
  afterEach(() => { _resetCodecRegistry(); });

  it('global codec: stringify auto-detects custom class and calls toPayload', async () => {
    await use({ decimal: {
      type: 'decimal',
      is(v: unknown): v is MyDecimal { return v instanceof MyDecimal; },
      toPayload(v: MyDecimal): string { return v.raw + 'm'; },
      fromPayload(payload: CodecPayload): MyDecimal {
        const raw = expectStringPayload(payload, 'decimal');
        const s = raw.endsWith('m') ? raw.slice(0, -1) : raw;
        return new MyDecimal(s);
      },
    } });

    const result = stringify({ price: new MyDecimal('99.99') });
    assert.ok(result.includes('99.99m'), `expected decimal in output, got: ${result}`);
  });

  it('later-registered codec overrides previous global codec (stringify)', async () => {
    await use({ decimal: {
      type: 'decimal',
      is(v: unknown): v is MyDecimal { return v instanceof MyDecimal; },
      toPayload(v: MyDecimal): string { return v.raw + 'm'; },
      fromPayload(_payload: CodecPayload): MyDecimal { return new MyDecimal(''); },
    } });

    const result = stringify({ price: new MyDecimal('42.5') });
    assert.ok(result.includes('42.5m'), `expected decimal toPayload output, got: ${result}`);
  });
});

// ─── date / time / datetime codec 系統 ──────────────────────────────────────

function createDateCodec(): Codec<string> {
  return {
    type: 'date' as const,
    fromPayload(payload: CodecPayload): string { return `DATE(${expectStringPayload(payload, 'date')})`; },
    toPayload(v: string): string { return v.replace(/^DATE\(|\)$/g, ''); },
    is(v: unknown): v is string { return typeof v === 'string' && v.startsWith('DATE('); },
  };
}

function createTimeCodec(): Codec<string> {
  return {
    type: 'time' as const,
    fromPayload(payload: CodecPayload): string { return `TIME(${expectStringPayload(payload, 'time')})`; },
    toPayload(v: string): string { return v.replace(/^TIME\(|\)$/g, ''); },
    is(v: unknown): v is string { return typeof v === 'string' && v.startsWith('TIME('); },
  };
}

function createDatetimeCodec(): Codec<number> {
  return {
    type: 'datetime' as const,
    fromPayload(payload: CodecPayload): number { return new Date(expectStringPayload(payload, 'datetime')).getTime(); },
    toPayload(v: number): string { return new Date(v).toISOString(); },
    is(v: unknown): v is number { return typeof v === 'number'; },
  };
}

describe('date/time/datetime codec fromPayload', () => {
  beforeEach(() => { _resetCodecRegistry(); });
  afterEach(() => { _resetCodecRegistry(); });

  it('date codec: irToJs returns custom object', () => {
    const node = irDate('2024-01-15');
    const result = irToJs(node, { date: createDateCodec() });
    assert.equal(result, 'DATE(2024-01-15)');
  });

  it('time codec: irToJs returns custom object', () => {
    const node = irTime('14:30:00');
    const result = irToJs(node, { time: createTimeCodec() });
    assert.equal(result, 'TIME(14:30:00)');
  });

  it('datetime codec: irToJs returns custom object', () => {
    const node = irDatetime('2024-01-15T12:00:00Z');
    const expectedMs = new Date('2024-01-15T12:00:00Z').getTime();
    const result = irToJs(node, { datetime: createDatetimeCodec() });
    assert.equal(result, expectedMs);
  });
});

// ─── date/time/datetime codec use() + per-call + precedence ─────────────────

describe('date/time/datetime codec system (use / per-call / precedence)', () => {
  beforeEach(() => { _resetCodecRegistry(); });
  afterEach(() => { _resetCodecRegistry(); });

  it('use() global date codec: parse returns custom value', async () => {
    await use({ date: createDateCodec() });
    const result = parse('born: 2024-01-15\n') as Record<string, unknown>;
    assert.equal(result.born, 'DATE(2024-01-15)');
  });

  it('use() global time codec: parse returns custom value', async () => {
    await use({ time: createTimeCodec() });
    const result = parse('start: 14:30:00\n') as Record<string, unknown>;
    assert.equal(result.start, 'TIME(14:30:00)');
  });

  it('use() global datetime codec: parse returns custom value', async () => {
    await use({ datetime: createDatetimeCodec() });
    const result = parse('ts: 2024-01-15T12:00:00Z\n') as Record<string, unknown>;
    assert.equal(result.ts, new Date('2024-01-15T12:00:00Z').getTime());
  });

  it('per-call date codec override (no use needed)', () => {
    const result = parse('born: 2024-01-15\n', {
      codecs: { date: createDateCodec() },
    }) as Record<string, unknown>;
    assert.equal(result.born, 'DATE(2024-01-15)');

    // 預設行為不受影響
    const defaultResult = parse('born: 2024-01-15\n') as Record<string, unknown>;
    assert.equal(defaultResult.born, '2024-01-15');
  });

  it('per-call time codec override (no use needed)', () => {
    const result = parse('start: 09:00:00\n', {
      codecs: { time: createTimeCodec() },
    }) as Record<string, unknown>;
    assert.equal(result.start, 'TIME(09:00:00)');

    const defaultResult = parse('start: 09:00:00\n') as Record<string, unknown>;
    assert.equal(defaultResult.start, '09:00:00');
  });

  it('per-call datetime codec override (no use needed)', () => {
    const ts = '2024-06-15T08:30:00Z';
    const result = parse(`ts: ${ts}\n`, {
      codecs: { datetime: createDatetimeCodec() },
    }) as Record<string, unknown>;
    assert.equal(result.ts, new Date(ts).getTime());

    const defaultResult = parse(`ts: ${ts}\n`) as Record<string, unknown>;
    assert.equal(defaultResult.ts, ts);
  });

  it('per-call codec takes precedence over global use() (date)', async () => {
    await use({ date: {
      type: 'date',
      fromPayload(payload: CodecPayload): string { return `GLOBAL(${expectStringPayload(payload, 'date')})`; },
      toPayload(v: string): string { return v; },
      is(v: unknown): v is string { return false; },
    } });

    const perCall = parse('born: 2024-01-15\n', {
      codecs: { date: createDateCodec() },
    }) as Record<string, unknown>;
    assert.equal(perCall.born, 'DATE(2024-01-15)');

    const globalResult = parse('born: 2024-01-15\n') as Record<string, unknown>;
    assert.equal(globalResult.born, 'GLOBAL(2024-01-15)');
  });

  it('per-call codec takes precedence over global use() (time)', async () => {
    await use({ time: {
      type: 'time',
      fromPayload(payload: CodecPayload): string { return `GLOBAL(${expectStringPayload(payload, 'time')})`; },
      toPayload(v: string): string { return v; },
      is(v: unknown): v is string { return false; },
    } });

    const perCall = parse('at: 10:00:00\n', {
      codecs: { time: createTimeCodec() },
    }) as Record<string, unknown>;
    assert.equal(perCall.at, 'TIME(10:00:00)');

    const globalResult = parse('at: 10:00:00\n') as Record<string, unknown>;
    assert.equal(globalResult.at, 'GLOBAL(10:00:00)');
  });

  it('per-call codec takes precedence over global use() (datetime)', async () => {
    await use({ datetime: {
      type: 'datetime',
      fromPayload(payload: CodecPayload): string { return `GLOBAL(${expectStringPayload(payload, 'datetime')})`; },
      toPayload(_v: number): string { return ''; },
      is(v: unknown): v is number { return false; },
    } });

    const ts = '2024-01-01T00:00:00Z';
    const perCall = parse(`ts: ${ts}\n`, {
      codecs: { datetime: createDatetimeCodec() },
    }) as Record<string, unknown>;
    assert.equal(perCall.ts, new Date(ts).getTime());

    const globalResult = parse(`ts: ${ts}\n`) as Record<string, unknown>;
    assert.equal(globalResult.ts, `GLOBAL(${ts})`);
  });
});

// ─── date/time/datetime stringify() codec is + toPayload ─────────────────────

class MyDate {
  constructor(public value: string) {}
}

class MyTime {
  constructor(public value: string) {}
}

class MyDatetime {
  constructor(public value: string) {}
}

describe('date/time/datetime stringify() codec is + toPayload', () => {
  beforeEach(() => { _resetCodecRegistry(); });
  afterEach(() => { _resetCodecRegistry(); });

  it('global date codec: stringify recognizes custom class', async () => {
    await use({ date: {
      type: 'date',
      is(v: unknown): v is MyDate { return v instanceof MyDate; },
      toPayload(v: MyDate): string { return v.value; },
      fromPayload(payload: CodecPayload): MyDate { return new MyDate(expectStringPayload(payload, 'date')); },
    } });

    const result = stringify({ born: new MyDate('2024-01-15') });
    assert.ok(result.includes('2024-01-15'), `expected date in output, got: ${result}`);
  });

  it('global time codec: stringify recognizes custom class', async () => {
    await use({ time: {
      type: 'time',
      is(v: unknown): v is MyTime { return v instanceof MyTime; },
      toPayload(v: MyTime): string { return v.value; },
      fromPayload(payload: CodecPayload): MyTime { return new MyTime(expectStringPayload(payload, 'time')); },
    } });

    const result = stringify({ start: new MyTime('14:30:00') });
    assert.ok(result.includes('14:30:00'), `expected time in output, got: ${result}`);
  });

  it('global datetime codec: stringify recognizes custom class', async () => {
    await use({ datetime: {
      type: 'datetime',
      is(v: unknown): v is MyDatetime { return v instanceof MyDatetime; },
      toPayload(v: MyDatetime): string { return v.value; },
      fromPayload(payload: CodecPayload): MyDatetime { return new MyDatetime(expectStringPayload(payload, 'datetime')); },
    } });

    const result = stringify({ ts: new MyDatetime('2024-06-15T08:30:00Z') });
    assert.ok(result.includes('2024-06-15'), `expected datetime in output, got: ${result}`);
  });
});

// ─── date/time/datetime end-to-end roundtrip with codec ──────────────────────

describe('date/time/datetime end-to-end roundtrip with codec', () => {
  beforeEach(() => { _resetCodecRegistry(); });
  afterEach(() => { _resetCodecRegistry(); });

  it('date roundtrip: parse → stringify → parse', async () => {
    await use({ date: createDateCodec() });

    const parsed = parse('born: 2024-03-15\n', { codecs: { date: createDateCodec() } }) as Record<string, unknown>;
    assert.equal(parsed.born, 'DATE(2024-03-15)');

    // stringify with toon.date tag (since the codec only is() on string starting with DATE()
    // and the string "DATE(2024-03-15)" doesn't have $type)
    const text = stringify({ born: { $type: 'date', value: '2024-03-15' } as never });
    const reparsed = parse(text, { codecs: { date: createDateCodec() } }) as Record<string, unknown>;
    assert.equal(reparsed.born, 'DATE(2024-03-15)');
  });

  it('time roundtrip: parse → stringify → parse', async () => {
    await use({ time: createTimeCodec() });

    const parsed = parse('at: 09:30:00\n', { codecs: { time: createTimeCodec() } }) as Record<string, unknown>;
    assert.equal(parsed.at, 'TIME(09:30:00)');

    const text = stringify({ at: { $type: 'time', value: '09:30:00' } as never });
    const reparsed = parse(text, { codecs: { time: createTimeCodec() } }) as Record<string, unknown>;
    assert.equal(reparsed.at, 'TIME(09:30:00)');
  });

  it('datetime roundtrip: parse → stringify → parse', async () => {
    const ts = '2024-01-15T12:00:00.000Z';

    const dtCodec: Codec<MyDatetime> = {
      type: 'datetime' as const,
      fromPayload(payload: CodecPayload): MyDatetime { return new MyDatetime(expectStringPayload(payload, 'datetime')); },
      toPayload(v: MyDatetime): string { return v.value; },
      is(v: unknown): v is MyDatetime { return v instanceof MyDatetime; },
    };

    // Register globally so stringify can use it
    await use({ datetime: dtCodec });

    const parsed = parse(`event: ${ts}\n`) as Record<string, unknown>;
    assert.ok(parsed.event instanceof MyDatetime);
    assert.equal((parsed.event as MyDatetime).value, ts);

    // stringify uses global codec to recognize MyDatetime → irDatetime
    const text = stringify({ event: parsed.event as never });
    const reparsed = parse(text) as Record<string, unknown>;
    assert.ok(reparsed.event instanceof MyDatetime);
    assert.equal((reparsed.event as MyDatetime).value, ts);
  });
});

// ─── 向後相容性 ──────────────────────────────────────────────────────────────

describe('Backward compatibility', () => {
  it('parse without options behaves unchanged', () => {
    const result = parse('price: 99.99m\n');
    assert.deepEqual(result, { price: '99.99' });
  });

  it('irToJs without options behaves unchanged', () => {
    const node = irDecimal('99.99m');
    const result = irToJs(node);
    assert.equal(result, '99.99');
  });

  it('non-decimal fields unaffected', () => {
    const input = `name: Alice\nage: 30\nprice: 99.99m\n`;
    const result = parse(input) as Record<string, unknown>;
    assert.equal(result.name, 'Alice');
    assert.equal(result.age, 30);
    assert.equal(result.price, '99.99');
  });
});
