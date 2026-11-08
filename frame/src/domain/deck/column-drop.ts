export type ColumnFrame = { id: string; x: number; y: number; width: number; height: number };
export type ColumnDrop = { before: string | null; row: 0 | 1; preview: [number, number, number, number] };

export function columnDrop(
  frames: readonly ColumnFrame[],
  columns: readonly { id: string; row: number }[],
  carried: string,
  at: { x: number; y: number },
  bounds: { left: number; top: number; right: number; bottom: number },
): ColumnDrop | null {
  if (at.x < bounds.left || at.x > bounds.right || at.y < bounds.top || at.y > bounds.bottom) return null;
  const upper = frames.filter((frame) => columns.some((column) => column.id === frame.id && column.row === 0));
  const lower = frames.filter((frame) => columns.some((column) => column.id === frame.id && column.row === 1));
  const split = columns.some((column) => column.row === 0) && columns.some((column) => column.row === 1);
  const middle = split && lower.length > 0 ? Math.min(...lower.map((frame) => frame.y)) : (bounds.top + bounds.bottom) / 2;
  const row: 0 | 1 = at.y >= middle ? 1 : 0;
  const candidates = (row === 0 ? upper : lower).filter((frame) => frame.id !== carried).sort((a, b) => a.x - b.x);
  const before = candidates.find((frame) => at.x < frame.x + frame.width / 2);
  const last = candidates.at(-1);
  const x = before?.x ?? (last ? last.x + last.width : bounds.left);
  const top = row === 1 ? middle : bounds.top;
  const bottom = row === 1 ? bounds.bottom : split ? middle : bounds.bottom;
  const creatingLower = !split && row === 1;
  return { before: before?.id ?? null, row, preview: creatingLower
    ? [bounds.left, middle, bounds.right - bounds.left, bounds.bottom - middle]
    : [Math.min(bounds.right - 2, Math.max(bounds.left, x)), top, 2, bottom - top] };
}
