import type { CodecType } from './codec.js';

type FieldKind =
  | 'string'
  | 'int'
  | 'float'
  | 'bool'
  | 'decimal'
  | 'date'
  | 'time'
  | 'datetime'
  | 'uuid'
  | 'binary';

export class FieldTypeSpec {
  constructor(
    public readonly kind: FieldKind,
    public readonly nullableFlag: boolean = false,
    public readonly precision?: number,
    public readonly scale?: number,
    public readonly hasTz?: boolean,
  ) {}

  nullable(): FieldTypeSpec {
    return new FieldTypeSpec(
      this.kind,
      true,
      this.precision,
      this.scale,
      this.hasTz,
    );
  }

  export(): Record<string, unknown> {
    const spec: Record<string, unknown> = {
      type: this.kind,
      nullable: this.nullableFlag,
    };
    if (this.precision !== undefined) spec['precision'] = this.precision;
    if (this.scale !== undefined) spec['scale'] = this.scale;
    if (this.hasTz !== undefined) spec['has_tz'] = this.hasTz;
    return spec;
  }

  get codecKey(): CodecType {
    return this.kind as CodecType;
  }
}

class TypesNamespace {
  readonly string = new FieldTypeSpec('string');
  readonly int = new FieldTypeSpec('int');
  readonly float = new FieldTypeSpec('float');
  readonly bool = new FieldTypeSpec('bool');
  readonly date = new FieldTypeSpec('date');
  readonly time = new FieldTypeSpec('time');
  readonly datetime = new FieldTypeSpec('datetime', false, undefined, undefined, true);
  readonly datetimeNaive = new FieldTypeSpec('datetime', false, undefined, undefined, false);
  readonly uuid = new FieldTypeSpec('uuid');
  readonly binary = new FieldTypeSpec('binary');

  decimal(precision: number, scale: number): FieldTypeSpec {
    return new FieldTypeSpec('decimal', false, Number(precision), Number(scale));
  }
}

export const types = new TypesNamespace();

export type StreamSchemaInput =
  | Record<string, FieldTypeSpec>
  | Iterable<[string, FieldTypeSpec]>;

export class StreamSchema implements Iterable<[string, FieldTypeSpec]> {
  private readonly fields = new Map<string, FieldTypeSpec>();

  constructor(input: StreamSchemaInput) {
    const entries = isRecord(input) ? Object.entries(input) : Array.from(input);
    for (const [name, fieldType] of entries) {
      if (typeof name !== 'string') {
        throw new TypeError('stream schema field name must be string');
      }
      if (!(fieldType instanceof FieldTypeSpec)) {
        throw new TypeError('stream schema field type must be built from ttoon.types');
      }
      if (this.fields.has(name)) {
        throw new Error(`duplicate stream schema field '${name}'`);
      }
      this.fields.set(name, fieldType);
    }
    if (this.fields.size === 0) {
      throw new Error('stream schema must contain at least one field');
    }
  }

  get(name: string): FieldTypeSpec | undefined {
    return this.fields.get(name);
  }

  entries(): IterableIterator<[string, FieldTypeSpec]> {
    return this.fields.entries();
  }

  keys(): IterableIterator<string> {
    return this.fields.keys();
  }

  values(): IterableIterator<FieldTypeSpec> {
    return this.fields.values();
  }

  [Symbol.iterator](): IterableIterator<[string, FieldTypeSpec]> {
    return this.entries();
  }

  export(): Array<Record<string, unknown>> {
    return Array.from(this.fields.entries(), ([name, fieldType]) => ({
      name,
      ...fieldType.export(),
    }));
  }
}

function isRecord(value: StreamSchemaInput): value is Record<string, FieldTypeSpec> {
  return typeof value === 'object'
    && value !== null
    && !Array.isArray(value)
    && typeof (value as Iterable<[string, FieldTypeSpec]>)[Symbol.iterator] !== 'function';
}
