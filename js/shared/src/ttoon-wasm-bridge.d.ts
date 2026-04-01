declare module 'ttoon-wasm-bridge' {
  export default function init(moduleOrPath?: unknown): Promise<unknown>;

  export function parse(text: string, mode: string): string;
  export function stringify_ttoon(irJson: string, optsJson: string): string;
  export function stringify_tjson(irJson: string, optsJson: string): string;
  export function tjson_to_ttoon(text: string, optsJson: string): string;
  export function ttoon_to_tjson(text: string, mode: string, optsJson: string): string;
  export function read_arrow(text: string): Uint8Array;
  export function stringify_arrow_ttoon(ipcBytes: Uint8Array, optsJson: string): string;
  export function stringify_arrow_tjson(ipcBytes: Uint8Array, optsJson: string): string;

  export class StreamObjectReaderHandle {
    constructor(schemaJson: string, mode?: string);
    feed(chunk: string): string;
    finish(): string;
    free(): void;
  }

  export class StreamArrowReaderHandle {
    constructor(schemaJson: string, batchSize: number, mode?: string);
    feed(chunk: string): Uint8Array;
    finish(): Uint8Array;
    free(): void;
  }

  export class StreamObjectTjsonReaderHandle {
    constructor(schemaJson: string, mode?: string);
    feed(chunk: string): string;
    finish(): string;
    free(): void;
  }

  export class StreamArrowTjsonReaderHandle {
    constructor(schemaJson: string, batchSize: number, mode?: string);
    feed(chunk: string): Uint8Array;
    finish(): Uint8Array;
    free(): void;
  }

  export class StreamObjectWriterHandle {
    constructor(schemaJson: string, optsJson?: string);
    write_row(rowJson: string): string;
    close(): string;
    readonly rows_emitted: number;
    free(): void;
  }

  export class StreamArrowWriterHandle {
    constructor(schemaJson: string, optsJson?: string);
    write_ipc(ipcBytes: Uint8Array): string;
    close(): string;
    readonly rows_emitted: number;
    free(): void;
  }

  export class StreamObjectTjsonWriterHandle {
    constructor(schemaJson: string, optsJson?: string);
    write_row(rowJson: string): string;
    close(): string;
    readonly rows_emitted: number;
    free(): void;
  }

  export class StreamArrowTjsonWriterHandle {
    constructor(schemaJson: string, optsJson?: string);
    write_ipc(ipcBytes: Uint8Array): string;
    close(): string;
    readonly rows_emitted: number;
    free(): void;
  }
}
