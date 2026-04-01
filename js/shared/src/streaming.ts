import type { RecordBatch } from 'apache-arrow';

import { getCodecs, type CodecPayload, type CodecRegistry, type CodecType, type IntPayload } from './codec.js';
import { irToJs, jsToIr } from './convert.js';
import { parseError, serializeError } from './errors.js';
import {
  irBinary,
  irDate,
  irDatetime,
  irDecimal,
  irInt,
  irInt64,
  irObject,
  irTime,
  irUuid,
  isValidI64,
  type IrNode,
} from './ir.js';
import { StreamSchema, type StreamSchemaInput } from './schema.js';
import type { ParseMode } from './typed_parse.js';
import {
  StreamArrowReaderHandle as WasmStreamArrowReaderHandle,
  StreamArrowTjsonReaderHandle as WasmStreamArrowTjsonReaderHandle,
  StreamArrowTjsonWriterHandle as WasmStreamArrowTjsonWriterHandle,
  StreamArrowWriterHandle as WasmStreamArrowWriterHandle,
  StreamObjectReaderHandle as WasmStreamObjectReaderHandle,
  StreamObjectTjsonReaderHandle as WasmStreamObjectTjsonReaderHandle,
  StreamObjectTjsonWriterHandle as WasmStreamObjectTjsonWriterHandle,
  StreamObjectWriterHandle as WasmStreamObjectWriterHandle,
} from './wasm_adapter.js';

type TextChunk = string | Uint8Array;

interface ReadableStreamReaderLike<T> {
  read(): Promise<{ done: boolean; value?: T }>;
  releaseLock?(): void;
}

interface ReadableStreamLike<T> {
  getReader(): ReadableStreamReaderLike<T>;
}

interface WritableStreamWriterLike<T> {
  write(chunk: T): Promise<void>;
  releaseLock?(): void;
}

interface WritableStreamLike<T> {
  getWriter(): WritableStreamWriterLike<T>;
}

export type TextSource =
  | string
  | Iterable<TextChunk>
  | AsyncIterable<TextChunk>
  | ReadableStreamLike<TextChunk>;

export type TextSink =
  | ((chunk: string) => void | Promise<void>)
  | { write(chunk: string): void | Promise<void> }
  | WritableStreamLike<string>;

export interface StreamResult {
  rowsEmitted: number;
}

export interface StreamReadOptions {
  schema: StreamSchema | StreamSchemaInput;
  mode?: ParseMode;
  codecs?: CodecRegistry;
}

export interface StreamReadArrowOptions {
  schema: StreamSchema | StreamSchemaInput;
  batchSize?: number;
  mode?: ParseMode;
}

export interface StreamWriteOptions {
  schema: StreamSchema | StreamSchemaInput;
  delimiter?: ',' | '\t' | '|';
  binaryFormat?: 'hex' | 'b64';
  codecs?: CodecRegistry;
}

export interface StreamWriteArrowOptions {
  schema: StreamSchema | StreamSchemaInput;
  delimiter?: ',' | '\t' | '|';
  binaryFormat?: 'hex' | 'b64';
}

export interface TjsonStreamWriteOptions {
  schema: StreamSchema | StreamSchemaInput;
  binaryFormat?: 'hex' | 'b64';
  codecs?: CodecRegistry;
}

export interface TjsonStreamWriteArrowOptions {
  schema: StreamSchema | StreamSchemaInput;
  binaryFormat?: 'hex' | 'b64';
}

class PendingActionQueue {
  private pending: Promise<void> = Promise.resolve();
  private error?: unknown;

  enqueue(action: () => void | Promise<void>): void {
    if (this.error !== undefined) {
      throw this.error;
    }
    this.pending = this.pending.then(action).catch((error: unknown) => {
      this.error ??= error;
      throw error;
    });
  }

  rethrowIfFailed(): void {
    if (this.error !== undefined) {
      throw this.error;
    }
  }

  async flush(): Promise<void> {
    this.rethrowIfFailed();
    await this.pending;
  }
}

export async function* streamRead(
  source: TextSource,
  options: StreamReadOptions,
): AsyncIterable<Record<string, unknown>> {
  const schema = normalizeSchema(options.schema);
  const handle = new WasmStreamObjectReaderHandle(
    JSON.stringify(schema.export()),
    options.mode ?? 'compat',
  );
  try {
    const codecs = snapshotCodecs(options.codecs);
    for await (const chunk of iterateTextSource(source, 'streamRead')) {
      yield* iterateObjectRows(handle.feed(chunk), codecs, 'streamRead');
    }
    yield* iterateObjectRows(handle.finish(), codecs, 'streamRead');
  } finally {
    handle.free();
  }
}

export async function* streamReadTjson(
  source: TextSource,
  options: StreamReadOptions,
): AsyncIterable<Record<string, unknown>> {
  const schema = normalizeSchema(options.schema);
  const handle = new WasmStreamObjectTjsonReaderHandle(
    JSON.stringify(schema.export()),
    options.mode ?? 'compat',
  );
  try {
    const codecs = snapshotCodecs(options.codecs);
    for await (const chunk of iterateTextSource(source, 'streamReadTjson')) {
      yield* iterateObjectRows(handle.feed(chunk), codecs, 'streamReadTjson');
    }
    yield* iterateObjectRows(handle.finish(), codecs, 'streamReadTjson');
  } finally {
    handle.free();
  }
}

export async function* streamReadArrow(
  source: TextSource,
  options: StreamReadArrowOptions,
): AsyncIterable<RecordBatch> {
  const schema = normalizeSchema(options.schema);
  const handle = new WasmStreamArrowReaderHandle(
    JSON.stringify(schema.export()),
    options.batchSize ?? 1024,
    options.mode ?? 'compat',
  );
  try {
    for await (const chunk of iterateTextSource(source, 'streamReadArrow')) {
      yield* iterateArrowBatches(handle.feed(chunk));
    }
    yield* iterateArrowBatches(handle.finish());
  } finally {
    handle.free();
  }
}

export async function* streamReadArrowTjson(
  source: TextSource,
  options: StreamReadArrowOptions,
): AsyncIterable<RecordBatch> {
  const schema = normalizeSchema(options.schema);
  const handle = new WasmStreamArrowTjsonReaderHandle(
    JSON.stringify(schema.export()),
    options.batchSize ?? 1024,
    options.mode ?? 'compat',
  );
  try {
    for await (const chunk of iterateTextSource(source, 'streamReadArrowTjson')) {
      yield* iterateArrowBatches(handle.feed(chunk));
    }
    yield* iterateArrowBatches(handle.finish());
  } finally {
    handle.free();
  }
}

export class StreamWriter {
  private readonly schema: StreamSchema;
  private readonly codecs: CodecRegistry;
  private readonly handle: WasmStreamObjectWriterHandle;
  private readonly queue: PendingActionQueue;
  private closed = false;
  private closeResult?: StreamResult;

  constructor(
    private readonly sink: TextSink,
    options: StreamWriteOptions,
  ) {
    this.schema = normalizeSchema(options.schema);
    this.codecs = snapshotCodecs(options.codecs);
    this.handle = new WasmStreamObjectWriterHandle(JSON.stringify(this.schema.export()), options);
    this.queue = new PendingActionQueue();
  }

  get result(): StreamResult | undefined {
    return this.closeResult;
  }

  write(row: Record<string, unknown>): void {
    ensureOpen(this.closed, 'streamWriter');
    this.queue.rethrowIfFailed();
    const text = this.handle.writeRow(encodeRowToIrObject(this.schema, row, this.codecs));
    this.queue.enqueue(() => writeTextSink(this.sink, text, 'streamWriter'));
  }

  async close(): Promise<StreamResult> {
    ensureOpen(this.closed, 'streamWriter');
    this.closed = true;
    try {
      this.queue.rethrowIfFailed();
      const text = this.handle.close();
      this.queue.enqueue(() => writeTextSink(this.sink, text, 'streamWriter'));
      await this.queue.flush();
      this.closeResult = { rowsEmitted: this.handle.rowsEmitted };
      return this.closeResult;
    } finally {
      this.handle.free();
    }
  }
}

export class TjsonStreamWriter {
  private readonly schema: StreamSchema;
  private readonly codecs: CodecRegistry;
  private readonly handle: WasmStreamObjectTjsonWriterHandle;
  private readonly queue: PendingActionQueue;
  private closed = false;
  private closeResult?: StreamResult;

  constructor(
    private readonly sink: TextSink,
    options: TjsonStreamWriteOptions,
  ) {
    this.schema = normalizeSchema(options.schema);
    this.codecs = snapshotCodecs(options.codecs);
    this.handle = new WasmStreamObjectTjsonWriterHandle(JSON.stringify(this.schema.export()), options);
    this.queue = new PendingActionQueue();
  }

  get result(): StreamResult | undefined {
    return this.closeResult;
  }

  write(row: Record<string, unknown>): void {
    ensureOpen(this.closed, 'streamWriterTjson');
    this.queue.rethrowIfFailed();
    const text = this.handle.writeRow(encodeRowToIrObject(this.schema, row, this.codecs));
    this.queue.enqueue(() => writeTextSink(this.sink, text, 'streamWriterTjson'));
  }

  async close(): Promise<StreamResult> {
    ensureOpen(this.closed, 'streamWriterTjson');
    this.closed = true;
    try {
      this.queue.rethrowIfFailed();
      const text = this.handle.close();
      this.queue.enqueue(() => writeTextSink(this.sink, text, 'streamWriterTjson'));
      await this.queue.flush();
      this.closeResult = { rowsEmitted: this.handle.rowsEmitted };
      return this.closeResult;
    } finally {
      this.handle.free();
    }
  }
}

export class ArrowStreamWriter {
  private readonly schema: StreamSchema;
  private readonly handle: WasmStreamArrowWriterHandle;
  private readonly queue: PendingActionQueue;
  private rowsEmitted = 0;
  private closed = false;
  private closeResult?: StreamResult;

  constructor(
    private readonly sink: TextSink,
    options: StreamWriteArrowOptions,
  ) {
    this.schema = normalizeSchema(options.schema);
    this.handle = new WasmStreamArrowWriterHandle(JSON.stringify(this.schema.export()), options);
    this.queue = new PendingActionQueue();
  }

  get result(): StreamResult | undefined {
    return this.closeResult;
  }

  writeBatch(batch: RecordBatch): void {
    ensureOpen(this.closed, 'streamWriterArrow');
    this.queue.rethrowIfFailed();
    this.rowsEmitted += batch.numRows;
    this.queue.enqueue(async () => {
      const ipcBytes = await recordBatchToIpcBytes(batch);
      const text = this.handle.writeIpc(ipcBytes);
      await writeTextSink(this.sink, text, 'streamWriterArrow');
    });
  }

  async close(): Promise<StreamResult> {
    ensureOpen(this.closed, 'streamWriterArrow');
    this.closed = true;
    try {
      this.queue.rethrowIfFailed();
      this.queue.enqueue(async () => {
        const text = this.handle.close();
        await writeTextSink(this.sink, text, 'streamWriterArrow');
      });
      await this.queue.flush();
      this.closeResult = { rowsEmitted: this.rowsEmitted };
      return this.closeResult;
    } finally {
      this.handle.free();
    }
  }
}

export class TjsonArrowStreamWriter {
  private readonly schema: StreamSchema;
  private readonly handle: WasmStreamArrowTjsonWriterHandle;
  private readonly queue: PendingActionQueue;
  private rowsEmitted = 0;
  private closed = false;
  private closeResult?: StreamResult;

  constructor(
    private readonly sink: TextSink,
    options: TjsonStreamWriteArrowOptions,
  ) {
    this.schema = normalizeSchema(options.schema);
    this.handle = new WasmStreamArrowTjsonWriterHandle(JSON.stringify(this.schema.export()), options);
    this.queue = new PendingActionQueue();
  }

  get result(): StreamResult | undefined {
    return this.closeResult;
  }

  writeBatch(batch: RecordBatch): void {
    ensureOpen(this.closed, 'streamWriterArrowTjson');
    this.queue.rethrowIfFailed();
    this.rowsEmitted += batch.numRows;
    this.queue.enqueue(async () => {
      const ipcBytes = await recordBatchToIpcBytes(batch);
      const text = this.handle.writeIpc(ipcBytes);
      await writeTextSink(this.sink, text, 'streamWriterArrowTjson');
    });
  }

  async close(): Promise<StreamResult> {
    ensureOpen(this.closed, 'streamWriterArrowTjson');
    this.closed = true;
    try {
      this.queue.rethrowIfFailed();
      this.queue.enqueue(async () => {
        const text = this.handle.close();
        await writeTextSink(this.sink, text, 'streamWriterArrowTjson');
      });
      await this.queue.flush();
      this.closeResult = { rowsEmitted: this.rowsEmitted };
      return this.closeResult;
    } finally {
      this.handle.free();
    }
  }
}

export function streamWriter(sink: TextSink, options: StreamWriteOptions): StreamWriter {
  return new StreamWriter(sink, options);
}

export function streamWriterTjson(
  sink: TextSink,
  options: TjsonStreamWriteOptions,
): TjsonStreamWriter {
  return new TjsonStreamWriter(sink, options);
}

export function streamWriterArrow(
  sink: TextSink,
  options: StreamWriteArrowOptions,
): ArrowStreamWriter {
  return new ArrowStreamWriter(sink, options);
}

export function streamWriterArrowTjson(
  sink: TextSink,
  options: TjsonStreamWriteArrowOptions,
): TjsonArrowStreamWriter {
  return new TjsonArrowStreamWriter(sink, options);
}

function normalizeSchema(schema: StreamSchema | StreamSchemaInput): StreamSchema {
  return schema instanceof StreamSchema ? schema : new StreamSchema(schema);
}

function snapshotCodecs(codecs?: CodecRegistry): CodecRegistry {
  const snapshot: CodecRegistry = {};
  for (const [key, codec] of getCodecs()) {
    snapshot[key] = codec;
  }
  return {
    ...snapshot,
    ...codecs,
  };
}

function ensureOpen(closed: boolean, context: string): void {
  if (closed) {
    throw serializeError(`${context}: writer is already closed`);
  }
}

function encodeRowToIrObject(
  schema: StreamSchema,
  row: Record<string, unknown>,
  codecs: CodecRegistry,
): IrNode {
  const map = new Map<string, IrNode>();
  for (const [key, value] of Object.entries(row)) {
    const field = schema.get(key);
    if (field == null || value == null) {
      map.set(key, jsToIr(value as never));
      continue;
    }

    const codec = codecs[field.codecKey];
    if (codec && codec.is(value)) {
      map.set(key, codecPayloadToIrNode(field.codecKey, codec.toPayload(value as never)));
      continue;
    }

    map.set(key, jsToIr(value as never));
  }
  return irObject(map);
}

async function* iterateObjectRows(
  rowsNode: IrNode,
  codecs: CodecRegistry,
  context: string,
): AsyncIterable<Record<string, unknown>> {
  if (rowsNode.k !== 'list') {
    throw parseError(`${context}: expected row list`);
  }

  for (const rowNode of rowsNode.v) {
    if (rowNode.k !== 'object') {
      throw parseError(`${context}: expected object row`);
    }
    const row = irToJs(rowNode, codecs);
    if (row === null || typeof row !== 'object' || Array.isArray(row) || row instanceof Uint8Array) {
      throw parseError(`${context}: expected object row`);
    }
    yield row as Record<string, unknown>;
  }
}

async function* iterateArrowBatches(ipcBytes: Uint8Array): AsyncIterable<RecordBatch> {
  if (ipcBytes.length === 0) {
    return;
  }
  const { RecordBatchReader } = await import('apache-arrow');
  const reader = RecordBatchReader.from(ipcBytes);
  for (const batch of reader) {
    yield batch;
  }
}

async function* iterateTextSource(source: TextSource, context: string): AsyncIterable<string> {
  if (typeof source === 'string') {
    if (source.length > 0) {
      yield source;
    }
    return;
  }
  if (isReadableStreamLike<TextChunk>(source)) {
    const reader = source.getReader();
    const decoder = new TextDecoder();
    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        const text = decodeChunk(value, decoder, context);
        if (text.length > 0) {
          yield text;
        }
      }
      const tail = decoder.decode();
      if (tail.length > 0) {
        yield tail;
      }
      return;
    } finally {
      reader.releaseLock?.();
    }
  }
  if (isAsyncIterable<TextChunk>(source)) {
    const decoder = new TextDecoder();
    for await (const chunk of source) {
      const text = decodeChunk(chunk, decoder, context);
      if (text.length > 0) {
        yield text;
      }
    }
    const tail = decoder.decode();
    if (tail.length > 0) {
      yield tail;
    }
    return;
  }
  if (isIterable<TextChunk>(source)) {
    const decoder = new TextDecoder();
    for (const chunk of source) {
      const text = decodeChunk(chunk, decoder, context);
      if (text.length > 0) {
        yield text;
      }
    }
    const tail = decoder.decode();
    if (tail.length > 0) {
      yield tail;
    }
    return;
  }
  throw parseError(`${context}: unsupported source`);
}

async function writeTextSink(sink: TextSink, text: string, context: string): Promise<void> {
  if (typeof sink === 'function') {
    await sink(text);
    return;
  }
  if (isWritableStreamLike<string>(sink)) {
    const writer = sink.getWriter();
    try {
      await writer.write(text);
    } finally {
      writer.releaseLock?.();
    }
    return;
  }
  if (typeof sink.write === 'function') {
    await sink.write(text);
    return;
  }
  throw serializeError(`${context}: unsupported sink`);
}

async function recordBatchToIpcBytes(batch: RecordBatch): Promise<Uint8Array> {
  const { RecordBatchStreamWriter } = await import('apache-arrow');
  return RecordBatchStreamWriter.writeAll([batch]).toUint8Array(true);
}

function decodeChunk(chunk: TextChunk | undefined, decoder: TextDecoder, context: string): string {
  if (chunk === undefined) {
    return '';
  }
  if (typeof chunk === 'string') {
    return chunk;
  }
  if (chunk instanceof Uint8Array) {
    return decoder.decode(chunk, { stream: true });
  }
  throw parseError(`${context}: unsupported source chunk`);
}

function isAsyncIterable<T>(value: unknown): value is AsyncIterable<T> {
  return typeof value === 'object' && value !== null
    && typeof (value as AsyncIterable<T>)[Symbol.asyncIterator] === 'function';
}

function isIterable<T>(value: unknown): value is Iterable<T> {
  return typeof value === 'object' && value !== null
    && typeof (value as Iterable<T>)[Symbol.iterator] === 'function';
}

function isReadableStreamLike<T>(value: unknown): value is ReadableStreamLike<T> {
  return typeof value === 'object' && value !== null
    && typeof (value as ReadableStreamLike<T>).getReader === 'function';
}

function isWritableStreamLike<T>(value: unknown): value is WritableStreamLike<T> {
  return typeof value === 'object' && value !== null
    && typeof (value as WritableStreamLike<T>).getWriter === 'function';
}

function codecPayloadToIrNode(type: CodecType, payload: CodecPayload): IrNode {
  switch (type) {
    case 'int':
      return intPayloadToIrNode(payload);
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

function intPayloadToIrNode(payload: CodecPayload): IrNode {
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

function expectIntPayload(payload: CodecPayload): IntPayload {
  if (typeof payload === 'string' || payload instanceof Uint8Array) {
    throw serializeError('int codec must return IntPayload');
  }
  return payload;
}

function expectStringPayload(
  type: Exclude<CodecType, 'int' | 'binary'>,
  payload: CodecPayload,
): string {
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
