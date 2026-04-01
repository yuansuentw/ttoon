/**
 * Codec 系統 — C01/C02
 *
 * 定義 Codec<T> 介面與全域 use() 注入機制。
 * Codec 讓第三方函式庫（Temporal、Day.js、Big.js 等）透過依賴注入方式
 * 處理特定 typed type 的序列化/反序列化。
 *
 * typed type 名稱：'null' | 'bool' | 'int' | 'float' | 'decimal' |
 *                  'string' | 'date' | 'time' | 'datetime' | 'uuid' | 'binary'
 */
/** 支援的 typed type 名稱 */
export type CodecType = 'int' | 'decimal' | 'date' | 'time' | 'datetime' | 'uuid' | 'binary';

/** Int codec 的 public payload，保留 int64 溢位資訊。 */
export interface IntPayload {
  readonly value: number;
  readonly value64?: bigint;
}

/** Codec 與 internal IR 之間的公開 payload 表面型別。 */
export type CodecPayload = IntPayload | string | Uint8Array;

// ─── Codec 介面（C01）────────────────────────────────────────────────────────

/**
 * Codec<T> 介面：處理特定 typed type 的雙向轉換
 *
 * @template T 對應的 JS 型別（如 Temporal.PlainDate、Big 等）
 *
 * @remarks
 * **Primitive 短路行為**：`jsToIr` 對 `number` 與 `bigint` 使用 `typeof` 快速路徑直接攔截，
 * 不會經過 Codec 的 `is()` / `toPayload()` 方法。因此內建的 int Codec（`intNumber`、`intBigInt`）
 * 其 `toPayload` 和 `is` 僅在處理使用者自訂 class wrapper 時觸發。
 * 自訂 Codec 若需攔截 primitive 值，應留意此行為。
 */
export interface Codec<T = unknown> {
  /** typed type 名稱（optional，供 runtime 驗證用）*/
  readonly type?: CodecType;

  /** typed payload → T（反序列化）*/
  fromPayload(payload: CodecPayload): T;

  /** T → typed payload（序列化）*/
  toPayload(value: T): CodecPayload;

  /** type guard：判斷 value 是否為此 Codec 負責的型別 */
  is(value: unknown): value is T;
}

/** Codec 註冊表型別 */
export type CodecRegistry = Partial<Record<CodecType, Codec>>;

// ─── 全域 Codec 註冊表（C02）─────────────────────────────────────────────────

const _registry = new Map<CodecType, Codec>();

/**
 * 全域注入 Codec 實作（C02）
 *
 * Codec 在 irToJs / jsToIr 中自動查詢。
 * use() 為 async 以支援 Codec 內部可能的非同步初始化（如 lazy import）。
 *
 * @example
 * await use({ decimal: myDecimalCodec })
 * await use({ date: dateCodec, decimal: decimalCodec })
 */
export async function use(codecs: CodecRegistry): Promise<void> {
  for (const key of Object.keys(codecs) as CodecType[]) {
    const codec = codecs[key];
    if (codec == null) continue;
    if (codec.type != null && codec.type !== key) {
      throw new Error(
        `Codec type mismatch: registered under "${key}" but codec declares type "${codec.type}"`,
      );
    }
    _registry.set(key, codec);
  }
}

/** 查詢已註冊的 Codec（供 irToJs/jsToIr 內部使用） */
export function getCodec(type: CodecType): Codec | undefined {
  return _registry.get(type);
}

/** 取得所有已註冊的 Codec（供 jsToIr 遍歷） */
export function getCodecs(): ReadonlyMap<CodecType, Codec> {
  return _registry;
}

/** 供測試用：重設 Codec 註冊表 */
export function _resetCodecRegistry(): void {
  _registry.clear();
}
