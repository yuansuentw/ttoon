import { strict as assert } from 'node:assert';
import { afterEach, beforeEach, describe, it } from 'node:test';
import { parse, stringify, use } from '../index.js';
import { _resetCodecRegistry, type Codec, type CodecPayload } from '../codec.js';

class UuidBox {
  constructor(
    public value: string,
    public source: string,
  ) {}
}

class BinaryBox {
  constructor(
    public value: Uint8Array,
    public source: string,
  ) {}
}

function expectStringPayload(payload: CodecPayload, label: string): string {
  if (typeof payload !== 'string') throw new Error(`not ${label}`);
  return payload;
}

function expectBinaryPayload(payload: CodecPayload): Uint8Array {
  if (!(payload instanceof Uint8Array)) throw new Error('not binary');
  return payload;
}

function createUuidCodec(source: string): Codec<UuidBox> {
  return {
    type: 'uuid' as const,
    fromPayload(payload: CodecPayload): UuidBox {
      return new UuidBox(expectStringPayload(payload, 'uuid'), source);
    },
    toPayload(value: UuidBox): string {
      return value.value;
    },
    is(value: unknown): value is UuidBox {
      return value instanceof UuidBox && value.source === source;
    },
  };
}

function createBinaryCodec(source: string): Codec<BinaryBox> {
  return {
    type: 'binary' as const,
    fromPayload(payload: CodecPayload): BinaryBox {
      return new BinaryBox(expectBinaryPayload(payload), source);
    },
    toPayload(value: BinaryBox): Uint8Array {
      return value.value;
    },
    is(value: unknown): value is BinaryBox {
      return value instanceof BinaryBox && value.source === source;
    },
  };
}

function toHex(bytes: Uint8Array): string {
  return Buffer.from(bytes).toString('hex');
}

describe('uuid/binary codec system (use / per-call / stringify)', () => {
  beforeEach(() => { _resetCodecRegistry(); });
  afterEach(() => { _resetCodecRegistry(); });

  it('use() global uuid codec: parse returns custom object', async () => {
    await use({ uuid: createUuidCodec('global') });
    const result = parse('id: uuid(550e8400-e29b-41d4-a716-446655440000)\n') as Record<string, unknown>;
    assert.ok(result.id instanceof UuidBox);
    assert.equal((result.id as UuidBox).source, 'global');
    assert.equal((result.id as UuidBox).value, '550e8400-e29b-41d4-a716-446655440000');
  });

  it('per-call uuid codec takes precedence over global use()', async () => {
    await use({ uuid: createUuidCodec('global') });

    const perCall = parse('id: uuid(550e8400-e29b-41d4-a716-446655440000)\n', {
      codecs: { uuid: createUuidCodec('local') },
    }) as Record<string, unknown>;
    assert.ok(perCall.id instanceof UuidBox);
    assert.equal((perCall.id as UuidBox).source, 'local');

    const globalResult = parse('id: uuid(550e8400-e29b-41d4-a716-446655440000)\n') as Record<string, unknown>;
    assert.ok(globalResult.id instanceof UuidBox);
    assert.equal((globalResult.id as UuidBox).source, 'global');
  });

  it('use() global binary codec: parse returns custom object', async () => {
    await use({ binary: createBinaryCodec('global') });
    const result = parse('data: hex(deadbeef)\n') as Record<string, unknown>;
    assert.ok(result.data instanceof BinaryBox);
    assert.equal((result.data as BinaryBox).source, 'global');
    assert.equal(toHex((result.data as BinaryBox).value), 'deadbeef');
  });

  it('per-call binary codec takes precedence over global use()', async () => {
    await use({ binary: createBinaryCodec('global') });

    const perCall = parse('data: hex(deadbeef)\n', {
      codecs: { binary: createBinaryCodec('local') },
    }) as Record<string, unknown>;
    assert.ok(perCall.data instanceof BinaryBox);
    assert.equal((perCall.data as BinaryBox).source, 'local');

    const globalResult = parse('data: hex(deadbeef)\n') as Record<string, unknown>;
    assert.ok(globalResult.data instanceof BinaryBox);
    assert.equal((globalResult.data as BinaryBox).source, 'global');
  });

  it('global uuid codec: stringify recognizes custom class and outputs uuid(...)', async () => {
    await use({ uuid: createUuidCodec('custom') });
    const text = stringify({
      id: new UuidBox('550e8400-e29b-41d4-a716-446655440000', 'custom'),
    });
    assert.ok(text.includes('uuid(550e8400-e29b-41d4-a716-446655440000)'));
  });

  it('global binary codec: stringify recognizes custom class and outputs hex(...)', async () => {
    await use({ binary: createBinaryCodec('custom') });
    const text = stringify({
      data: new BinaryBox(Uint8Array.from([0xde, 0xad, 0xbe, 0xef]), 'custom'),
    });
    assert.ok(text.includes('hex(deadbeef)'));
  });
});
