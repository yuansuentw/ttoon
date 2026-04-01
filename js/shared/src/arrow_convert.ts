/**
 * Arrow Table 判斷
 *
 * 先前的 IR <-> Arrow Table 低階轉換已移除。
 * 目前正式 Arrow 路徑統一走 Arrow IPC bytes bridge。
 */

export function isArrowTable(value: unknown): boolean {
  if (value === null || typeof value !== 'object') return false;
  const v = value as Record<string, unknown>;
  return (
    typeof v.schema === 'object' && v.schema !== null &&
    typeof v.numRows === 'number' &&
    typeof v.getChild === 'function'
  );
}
