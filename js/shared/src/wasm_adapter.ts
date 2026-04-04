/**
 * WASM adapter — wraps ttoon-wasm-bridge functions with IrNode-compatible signatures.
 *
 * Data flow:
 *   Parse:     text → WASM parse → wire JSON → wireToIr → IrNode
 *   Stringify: IrNode → irToWire → JSON string → WASM stringify → text
 *   Transcode: text → WASM transcode → text (no JS intermediate)
 *   Arrow:     text → WASM → IPC bytes ↔ JS apache-arrow
 */
import initWasmBridgeRaw, {
  parse as wasmParseRaw,
  stringify_ttoon as wasmStringifyTtoonRaw,
  stringify_tjson as wasmStringifyTjsonRaw,
  tjson_to_ttoon as wasmTjsonToTtoonRaw,
  ttoon_to_tjson as wasmTtoonToTjsonRaw,
  read_arrow as wasmReadArrowRaw,
  stringify_arrow_ttoon as wasmStringifyArrowTtoonRaw,
  stringify_arrow_tjson as wasmStringifyArrowTjsonRaw,
  StreamArrowReaderHandle as WasmStreamArrowReaderHandleRaw,
  StreamArrowTjsonReaderHandle as WasmStreamArrowTjsonReaderHandleRaw,
  StreamArrowTjsonWriterHandle as WasmStreamArrowTjsonWriterHandleRaw,
  StreamArrowWriterHandle as WasmStreamArrowWriterHandleRaw,
  StreamObjectReaderHandle as WasmStreamObjectReaderHandleRaw,
  StreamObjectTjsonReaderHandle as WasmStreamObjectTjsonReaderHandleRaw,
  StreamObjectTjsonWriterHandle as WasmStreamObjectTjsonWriterHandleRaw,
  StreamObjectWriterHandle as WasmStreamObjectWriterHandleRaw,
} from 'ttoon-wasm-bridge';
import { wireToIr, irToWire } from './wasm_wire.js';
import type { SerializeOptions } from './ttoon_serializer.js';
import type { TjsonSerializeOptions } from './tjson_serializer.js';
import type { IrNode } from './ir.js';

const WASM_INIT_REQUIRED_MESSAGE =
  'ttoon-wasm-bridge is not initialized. Call initWasm() before using @ttoon/shared WASM APIs.';

let wasmReady = false;
let wasmInitPromise: Promise<void> | undefined;

type WasmInitInput = Parameters<typeof initWasmBridgeRaw>[0];

async function resolveDefaultWasmInput(): Promise<WasmInitInput | undefined> {
  if (!(typeof process !== 'undefined' && process.versions?.node)) {
    return undefined;
  }
  const { readFile } = await import('node:fs/promises');
  return readFile(new URL('../dist/ttoon_wasm_bridge_bg.wasm', import.meta.url));
}

export function isWasmInitialized(): boolean {
  return wasmReady;
}

export async function initWasm(input?: WasmInitInput): Promise<void> {
  if (wasmReady) {
    return;
  }
  const resolvedInput = input ?? await resolveDefaultWasmInput();
  const initArg = resolvedInput === undefined ? undefined : { module_or_path: resolvedInput };
  const promise = wasmInitPromise ??= initWasmBridgeRaw(initArg as WasmInitInput)
    .then(() => {
      wasmReady = true;
    })
    .catch((error: unknown) => {
      wasmInitPromise = undefined;
      throw error;
    });
  await promise;
}

function assertWasmReady(): void {
  if (!wasmReady) {
    throw new Error(WASM_INIT_REQUIRED_MESSAGE);
  }
}

// ─── Options conversion ─────────────────────────────────────────────────────

function serializeOptsToJson(opts?: SerializeOptions): string {
  if (!opts) return '';
  const wire: Record<string, unknown> = {};
  if (opts.delimiter !== undefined) wire.delimiter = opts.delimiter;
  if (opts.indentSize !== undefined) wire.indent_size = opts.indentSize;
  if (opts.binaryFormat !== undefined) wire.binary_format = opts.binaryFormat;
  return JSON.stringify(wire);
}

function tjsonOptsToJson(opts?: TjsonSerializeOptions): string {
  if (!opts) return '';
  const wire: Record<string, unknown> = {};
  if (opts.binaryFormat !== undefined) wire.binary_format = opts.binaryFormat;
  return JSON.stringify(wire);
}

function wireJsonToIr(wireJson: string): IrNode {
  return wireToIr(JSON.parse(wireJson));
}

function irNodeToWireJson(ir: IrNode): string {
  return JSON.stringify(irToWire(ir));
}

// ─── Parse (JSON wire) ─────────────────────────────────────────────────────

/** Parse any text (auto-detects T-TOON / T-JSON) → IrNode */
export function wasmParse(text: string, mode: string = 'compat'): IrNode {
  assertWasmReady();
  return wireJsonToIr(wasmParseRaw(text, mode));
}

// ─── Stringify (JSON wire) ─────────────────────────────────────────────────

/** Serialize IrNode → T-TOON text */
export function wasmStringifyTtoon(ir: IrNode, opts?: SerializeOptions): string {
  assertWasmReady();
  return wasmStringifyTtoonRaw(irNodeToWireJson(ir), serializeOptsToJson(opts));
}

/** Serialize IrNode → T-JSON text */
export function wasmStringifyTjson(ir: IrNode, opts?: TjsonSerializeOptions): string {
  assertWasmReady();
  return wasmStringifyTjsonRaw(irNodeToWireJson(ir), tjsonOptsToJson(opts));
}

// ─── Direct Transcode ───────────────────────────────────────────────────────

/** T-JSON text → T-TOON text (no JS intermediate) */
export function wasmTjsonToTtoon(text: string, opts?: SerializeOptions): string {
  assertWasmReady();
  return wasmTjsonToTtoonRaw(text, serializeOptsToJson(opts));
}

/** T-TOON text → T-JSON text (no JS intermediate) */
export function wasmTtoonToTjson(text: string, mode: string = 'compat', opts?: TjsonSerializeOptions): string {
  assertWasmReady();
  return wasmTtoonToTjsonRaw(text, mode, tjsonOptsToJson(opts));
}

// ─── Arrow IPC ──────────────────────────────────────────────────────────────

/** Parse text → Arrow IPC stream bytes */
export function wasmReadArrow(text: string): Uint8Array {
  assertWasmReady();
  return wasmReadArrowRaw(text);
}

/** Arrow IPC bytes → T-TOON tabular text */
export function wasmStringifyArrowTtoon(ipcBytes: Uint8Array, opts?: SerializeOptions): string {
  assertWasmReady();
  return wasmStringifyArrowTtoonRaw(ipcBytes, serializeOptsToJson(opts));
}

/** Arrow IPC bytes → T-JSON text */
export function wasmStringifyArrowTjson(ipcBytes: Uint8Array, opts?: TjsonSerializeOptions): string {
  assertWasmReady();
  return wasmStringifyArrowTjsonRaw(ipcBytes, tjsonOptsToJson(opts));
}

// ─── Streaming ───────────────────────────────────────────────────────────────

export class StreamObjectReaderHandle {
  private readonly raw: WasmStreamObjectReaderHandleRaw;

  constructor(schemaJson: string, mode: string = 'compat') {
    assertWasmReady();
    this.raw = new WasmStreamObjectReaderHandleRaw(schemaJson, mode);
  }

  feed(chunk: string): IrNode {
    return wireJsonToIr(this.raw.feed(chunk));
  }

  finish(): IrNode {
    return wireJsonToIr(this.raw.finish());
  }

  free(): void {
    this.raw.free();
  }
}

export class StreamArrowReaderHandle {
  private readonly raw: WasmStreamArrowReaderHandleRaw;

  constructor(schemaJson: string, batchSize: number, mode: string = 'compat') {
    assertWasmReady();
    this.raw = new WasmStreamArrowReaderHandleRaw(schemaJson, batchSize, mode);
  }

  feed(chunk: string): Uint8Array {
    return this.raw.feed(chunk);
  }

  finish(): Uint8Array {
    return this.raw.finish();
  }

  free(): void {
    this.raw.free();
  }
}

export class StreamObjectTjsonReaderHandle {
  private readonly raw: WasmStreamObjectTjsonReaderHandleRaw;

  constructor(schemaJson: string, mode: string = 'compat') {
    assertWasmReady();
    this.raw = new WasmStreamObjectTjsonReaderHandleRaw(schemaJson, mode);
  }

  feed(chunk: string): IrNode {
    return wireJsonToIr(this.raw.feed(chunk));
  }

  finish(): IrNode {
    return wireJsonToIr(this.raw.finish());
  }

  free(): void {
    this.raw.free();
  }
}

export class StreamArrowTjsonReaderHandle {
  private readonly raw: WasmStreamArrowTjsonReaderHandleRaw;

  constructor(schemaJson: string, batchSize: number, mode: string = 'compat') {
    assertWasmReady();
    this.raw = new WasmStreamArrowTjsonReaderHandleRaw(schemaJson, batchSize, mode);
  }

  feed(chunk: string): Uint8Array {
    return this.raw.feed(chunk);
  }

  finish(): Uint8Array {
    return this.raw.finish();
  }

  free(): void {
    this.raw.free();
  }
}

export class StreamObjectWriterHandle {
  private readonly raw: WasmStreamObjectWriterHandleRaw;

  constructor(schemaJson: string, opts?: SerializeOptions) {
    assertWasmReady();
    this.raw = new WasmStreamObjectWriterHandleRaw(schemaJson, serializeOptsToJson(opts));
  }

  writeRow(row: IrNode): string {
    return this.raw.write_row(irNodeToWireJson(row));
  }

  close(): string {
    return this.raw.close();
  }

  get rowsEmitted(): number {
    return this.raw.rows_emitted;
  }

  free(): void {
    this.raw.free();
  }
}

export class StreamArrowWriterHandle {
  private readonly raw: WasmStreamArrowWriterHandleRaw;

  constructor(schemaJson: string, opts?: SerializeOptions) {
    assertWasmReady();
    this.raw = new WasmStreamArrowWriterHandleRaw(schemaJson, serializeOptsToJson(opts));
  }

  writeIpc(ipcBytes: Uint8Array): string {
    return this.raw.write_ipc(ipcBytes);
  }

  close(): string {
    return this.raw.close();
  }

  get rowsEmitted(): number {
    return this.raw.rows_emitted;
  }

  free(): void {
    this.raw.free();
  }
}

export class StreamObjectTjsonWriterHandle {
  private readonly raw: WasmStreamObjectTjsonWriterHandleRaw;

  constructor(schemaJson: string, opts?: TjsonSerializeOptions) {
    assertWasmReady();
    this.raw = new WasmStreamObjectTjsonWriterHandleRaw(schemaJson, tjsonOptsToJson(opts));
  }

  writeRow(row: IrNode): string {
    return this.raw.write_row(irNodeToWireJson(row));
  }

  close(): string {
    return this.raw.close();
  }

  get rowsEmitted(): number {
    return this.raw.rows_emitted;
  }

  free(): void {
    this.raw.free();
  }
}

export class StreamArrowTjsonWriterHandle {
  private readonly raw: WasmStreamArrowTjsonWriterHandleRaw;

  constructor(schemaJson: string, opts?: TjsonSerializeOptions) {
    assertWasmReady();
    this.raw = new WasmStreamArrowTjsonWriterHandleRaw(schemaJson, tjsonOptsToJson(opts));
  }

  writeIpc(ipcBytes: Uint8Array): string {
    return this.raw.write_ipc(ipcBytes);
  }

  close(): string {
    return this.raw.close();
  }

  get rowsEmitted(): number {
    return this.raw.rows_emitted;
  }

  free(): void {
    this.raw.free();
  }
}
