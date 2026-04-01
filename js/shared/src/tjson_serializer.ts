/**
 * T-JSON 序列化選項
 *
 * 實際序列化由 WASM bridge (wasmStringifyTjson) 執行。
 * 此檔案僅保留公開 API 所需的型別定義。
 */

export interface TjsonSerializeOptions {
  /** Binary 輸出格式，預設 'hex' */
  binaryFormat?: 'hex' | 'b64';
}
