/**
 * T-TOON 序列化選項
 *
 * 實際序列化由 WASM bridge (wasmStringifyTtoon) 執行。
 * 此檔案僅保留公開 API 所需的型別定義。
 */

export interface SerializeOptions {
  /** 縮排空白數，預設 2 */
  indentSize?: number;
  /** 分隔符，預設 ',' */
  delimiter?: ',' | '\t' | '|';
  /** Binary 輸出格式，預設 'hex' */
  binaryFormat?: 'hex' | 'b64';
}
