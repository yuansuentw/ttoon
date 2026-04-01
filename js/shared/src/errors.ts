/** T-TOON 錯誤類型 */
export type ErrorKind = 'parse' | 'serialize' | 'detect' | 'transcode';
export type SourceErrorKind = Exclude<ErrorKind, 'transcode'>;
export type TranscodeOperation = 'tjson_to_ttoon' | 'ttoon_to_tjson';
export type TranscodePhase = 'parse' | 'serialize';

interface ToonErrorOptions {
  cause?: unknown;
}

export class ToonError extends Error {
  declare cause?: unknown;

  constructor(
    message: string,
    public readonly kind: ErrorKind,
    options?: ToonErrorOptions,
  ) {
    super(message);
    this.name = 'ToonError';
    if (options && 'cause' in options) {
      this.cause = options.cause;
    }
  }
}

export class TranscodeError extends ToonError {
  public readonly source: ToonError;
  public readonly sourceKind: SourceErrorKind;

  constructor(
    public readonly operation: TranscodeOperation,
    public readonly phase: TranscodePhase,
    source: ToonError,
  ) {
    super(`${operation}: ${phase} phase failed: ${source.message}`, 'transcode', { cause: source });
    this.name = 'TranscodeError';
    this.source = source instanceof TranscodeError ? source.source : source;
    this.sourceKind = sourceKindOf(source);
  }
}

export function parseError(msg: string): ToonError {
  return new ToonError(msg, 'parse');
}

export function serializeError(msg: string): ToonError {
  return new ToonError(msg, 'serialize');
}

function normalizeSourceError(error: unknown, fallbackKind: SourceErrorKind): ToonError {
  if (error instanceof ToonError) {
    return error;
  }
  const message = error instanceof Error ? error.message : String(error);
  return new ToonError(message, fallbackKind, error instanceof Error ? { cause: error } : undefined);
}

function sourceKindOf(error: ToonError): SourceErrorKind {
  if (error instanceof TranscodeError) {
    return error.sourceKind;
  }
  if (error.kind === 'transcode') {
    return 'parse';
  }
  return error.kind;
}

export function transcodeError(
  operation: TranscodeOperation,
  phase: TranscodePhase,
  error: unknown,
): TranscodeError {
  return new TranscodeError(
    operation,
    phase,
    normalizeSourceError(error, phase === 'parse' ? 'parse' : 'serialize'),
  );
}
