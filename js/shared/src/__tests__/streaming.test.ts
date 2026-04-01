import { strict as assert } from 'node:assert';
import { describe, it } from 'node:test';
import * as arrow from 'apache-arrow';

import {
  StreamSchema,
  streamRead,
  streamReadArrow,
  streamReadArrowTjson,
  streamReadTjson,
  streamWriter,
  streamWriterArrow,
  streamWriterArrowTjson,
  streamWriterTjson,
  types,
} from '../index.js';

async function collectAsync<T>(iterable: AsyncIterable<T>): Promise<T[]> {
  const items: T[] = [];
  for await (const item of iterable) {
    items.push(item);
  }
  return items;
}

async function waitForAsyncWork(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

async function waitWithTimeout<T>(promise: Promise<T>, timeoutMs: number = 100): Promise<T> {
  return Promise.race([
    promise,
    new Promise<T>((_, reject) => {
      setTimeout(() => reject(new Error(`timed out after ${timeoutMs}ms`)), timeoutMs);
    }),
  ]);
}

describe('streaming public API', () => {
  it('StreamSchema / types builds schema-first surface', () => {
    const schema = new StreamSchema({
      name: types.string,
      age: types.int.nullable(),
    });

    assert.deepEqual(schema.export(), [
      { name: 'name', type: 'string', nullable: false },
      { name: 'age', type: 'int', nullable: true },
    ]);
  });

  it('streamWriter / streamRead roundtrip T-TOON object path', async () => {
    const schema = new StreamSchema({
      name: types.string,
      age: types.int.nullable(),
    });
    let output = '';

    const writer = streamWriter({ write(chunk: string) { output += chunk; } }, { schema });
    writer.write({ name: 'Alice', age: 30 });
    const result = await writer.close();

    assert.equal(output, '[*]{name,age}:\n"Alice", 30\n');
    assert.deepEqual(result, { rowsEmitted: 1 });
    assert.deepEqual(writer.result, { rowsEmitted: 1 });

    const rows = await collectAsync(streamRead(output, { schema }));
    assert.deepEqual(rows, [{ name: 'Alice', age: 30 }]);
  });

  it('streamWriter outputs header and row before close', async () => {
    const schema = new StreamSchema({
      name: types.string,
      age: types.int.nullable(),
    });
    let output = '';

    const writer = streamWriter({ write(chunk: string) { output += chunk; } }, { schema });
    writer.write({ name: 'Alice', age: 30 });
    await waitForAsyncWork();

    assert.equal(output, '[*]{name,age}:\n"Alice", 30\n');

    const result = await writer.close();
    assert.equal(output, '[*]{name,age}:\n"Alice", 30\n');
    assert.deepEqual(result, { rowsEmitted: 1 });
  });

  it('streamWriter outputs exact-count empty header on 0-row', async () => {
    const schema = new StreamSchema({
      name: types.string,
      age: types.int,
    });
    let output = '';

    const writer = streamWriter({ write(chunk: string) { output += chunk; } }, { schema });
    const result = await writer.close();

    assert.equal(output, '[0]{name,age}:\n');
    assert.deepEqual(result, { rowsEmitted: 0 });
  });

  it('streamRead yields first row before source ends', async () => {
    const schema = new StreamSchema({
      name: types.string,
      age: types.int.nullable(),
    });
    let releaseSecondChunk!: () => void;
    let secondChunkRequested = false;
    const secondChunkGate = new Promise<void>((resolve) => {
      releaseSecondChunk = resolve;
    });

    async function* source(): AsyncIterable<string> {
      yield '[*]{name,age}:\n"Alice", 1\n';
      secondChunkRequested = true;
      await secondChunkGate;
      yield '"Bob", 2\n';
    }

    const iterator = streamRead(source(), { schema })[Symbol.asyncIterator]();
    const first = await iterator.next();

    assert.equal(first.done, false);
    assert.deepEqual(first.value, { name: 'Alice', age: 1 });
    assert.equal(secondChunkRequested, false);

    const secondPromise = iterator.next();
    await waitForAsyncWork();
    assert.equal(secondChunkRequested, true);

    releaseSecondChunk();
    const second = await secondPromise;

    assert.equal(second.done, false);
    assert.deepEqual(second.value, { name: 'Bob', age: 2 });
    assert.deepEqual(await iterator.next(), { done: true, value: undefined });
  });

  it('streamReadArrow / streamWriterArrow roundtrip Arrow path', async () => {
    const schema = new StreamSchema({
      name: types.string,
      age: types.int.nullable(),
    });
    const batches = await collectAsync(streamReadArrow('[*]{name,age}:\n"Alice", 1\n"Bob", 2\n', {
      schema,
      batchSize: 1,
    }));

    assert.equal(batches.length, 2);
    assert.equal(batches[0]!.numRows, 1);
    assert.equal(batches[1]!.numRows, 1);
    assert.equal(batches[0]!.getChild('name')!.get(0), 'Alice');
    assert.equal(batches[0]!.getChild('age')!.get(0), 1n);

    const table = new arrow.Table({
      name: arrow.vectorFromArray(['Alice', 'Bob'], new arrow.Utf8()),
      age: arrow.vectorFromArray([1n, null], new arrow.Int64()),
    });
    let output = '';

    const writer = streamWriterArrow({ write(chunk: string) { output += chunk; } }, { schema });
    writer.writeBatch(table.batches[0]!);
    const result = await writer.close();

    assert.equal(output, '[*]{name,age}:\n"Alice", 1\n"Bob", null\n');
    assert.deepEqual(result, { rowsEmitted: 2 });
  });

  it('streamWriterArrow outputs batch before close', async () => {
    const schema = new StreamSchema({
      name: types.string,
      age: types.int.nullable(),
    });
    const table = new arrow.Table({
      name: arrow.vectorFromArray(['Alice'], new arrow.Utf8()),
      age: arrow.vectorFromArray([1n], new arrow.Int64()),
    });
    let output = '';
    let resolveFirstWrite!: () => void;
    const firstWrite = new Promise<void>((resolve) => {
      resolveFirstWrite = resolve;
    });

    const writer = streamWriterArrow({
      write(chunk: string) {
        output += chunk;
        resolveFirstWrite();
      },
    }, { schema });
    writer.writeBatch(table.batches[0]!);
    await waitWithTimeout(firstWrite);

    assert.equal(output, '[*]{name,age}:\n"Alice", 1\n');

    const result = await writer.close();
    assert.equal(output, '[*]{name,age}:\n"Alice", 1\n');
    assert.deepEqual(result, { rowsEmitted: 1 });
  });

  it('streamWriterTjson / streamReadTjson supports nullable and missing-field materialization', async () => {
    const schema = new StreamSchema({
      name: types.string,
      age: types.int.nullable(),
    });
    let output = '';

    const writer = streamWriterTjson({ write(chunk: string) { output += chunk; } }, { schema });
    writer.write({ name: 'Alice', age: null });
    const result = await writer.close();

    assert.equal(output, '[{"name": "Alice", "age": null}]');
    assert.deepEqual(result, { rowsEmitted: 1 });

    const rows = await collectAsync(streamReadTjson(output, { schema }));
    assert.deepEqual(rows, [{ name: 'Alice', age: null }]);

    const materialized = await collectAsync(streamReadTjson('[{"name": "Bob"}]', { schema }));
    assert.deepEqual(materialized, [{ name: 'Bob', age: null }]);
  });

  it('streamReadArrow batchSize accumulates across chunks', async () => {
    const schema = new StreamSchema({
      name: types.string,
      age: types.int,
    });

    async function* twoChunks() {
      yield '[*]{name,age}:\n"Alice", 1\n';
      yield '"Bob", 2\n';
    }

    const batches = await collectAsync(streamReadArrow(twoChunks(), {
      schema,
      batchSize: 2,
    }));

    // batchSize=2 且總共 2 rows → 應累積成 1 個 batch
    assert.equal(batches.length, 1, 'batchSize=2 across chunks should accumulate into 1 batch, not produce 1 batch per feed');
    assert.equal(batches[0]!.numRows, 2);
    assert.equal(batches[0]!.getChild('name')!.get(0), 'Alice');
    assert.equal(batches[0]!.getChild('name')!.get(1), 'Bob');
  });

  it('streamReadArrowTjson batchSize accumulates across chunks', async () => {
    const schema = new StreamSchema({
      name: types.string,
      age: types.int,
    });

    async function* twoChunks() {
      yield '[{"name":"Alice","age":1}';
      yield ',{"name":"Bob","age":2}]';
    }

    const batches = await collectAsync(streamReadArrowTjson(twoChunks(), {
      schema,
      batchSize: 2,
    }));

    // batchSize=2 且總共 2 rows → 應累積成 1 個 batch
    assert.equal(batches.length, 1, 'batchSize=2 across chunks should accumulate into 1 batch, not produce 1 batch per feed');
    assert.equal(batches[0]!.numRows, 2);
    assert.equal(batches[0]!.getChild('name')!.get(0), 'Alice');
    assert.equal(batches[0]!.getChild('name')!.get(1), 'Bob');
  });

  it('streamReadArrowTjson / streamWriterArrowTjson roundtrip T-JSON Arrow path', async () => {
    const schema = new StreamSchema({
      name: types.string,
      age: types.int.nullable(),
    });
    const batches = await collectAsync(streamReadArrowTjson(
      '[{"name": "Alice", "age": 1}, {"name": "Bob", "age": null}]',
      { schema, batchSize: 1 },
    ));

    assert.equal(batches.length, 2);
    assert.equal(batches[0]!.getChild('name')!.get(0), 'Alice');
    assert.equal(batches[1]!.getChild('age')!.get(0), null);

    const table = new arrow.Table({
      name: arrow.vectorFromArray(['Alice', 'Bob'], new arrow.Utf8()),
      age: arrow.vectorFromArray([1n, null], new arrow.Int64()),
    });
    let output = '';

    const writer = streamWriterArrowTjson({ write(chunk: string) { output += chunk; } }, { schema });
    writer.writeBatch(table.batches[0]!);
    const result = await writer.close();

    assert.equal(output, '[{"name": "Alice", "age": 1}, {"name": "Bob", "age": null}]');
    assert.deepEqual(result, { rowsEmitted: 2 });
  });
});
