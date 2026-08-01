import type { NavigationDirection } from "./navigation-types";

export interface VirtualGridWindow {
  start: number;
  end: number;
}

export interface VirtualGridWindowConfig {
  totalItems: number;
  columns: number;
  visibleRows: number;
  overscanRows: number;
}

function assertGrid(columns: number): void {
  if (!Number.isInteger(columns) || columns < 1) {
    throw new Error("Virtual grid columns must be a positive integer");
  }
}

function assertIndex(index: number, totalItems: number): void {
  if (!Number.isInteger(index) || index < 0 || index >= totalItems) {
    throw new Error("Virtual grid index is outside the item range");
  }
}

export function getRow(index: number, columns: number): number {
  assertGrid(columns);
  if (!Number.isInteger(index) || index < 0) {
    throw new Error("Virtual grid index must be a non-negative integer");
  }
  return Math.floor(index / columns);
}

export function getColumn(index: number, columns: number): number {
  assertGrid(columns);
  if (!Number.isInteger(index) || index < 0) {
    throw new Error("Virtual grid index must be a non-negative integer");
  }
  return index % columns;
}

export function getIndex(row: number, column: number, columns: number): number {
  assertGrid(columns);
  if (!Number.isInteger(row) || row < 0) {
    throw new Error("Virtual grid row must be a non-negative integer");
  }
  if (!Number.isInteger(column) || column < 0 || column >= columns) {
    throw new Error("Virtual grid column is outside the row");
  }
  return row * columns + column;
}

export function getDirectionalTarget(
  index: number,
  direction: NavigationDirection,
  totalItems: number,
  columns: number,
): number | null {
  assertGrid(columns);
  if (!Number.isInteger(totalItems) || totalItems < 0) {
    throw new Error("Virtual grid totalItems must be a non-negative integer");
  }
  if (totalItems === 0) return null;
  assertIndex(index, totalItems);

  const row = getRow(index, columns);
  const column = getColumn(index, columns);
  let targetRow = row;
  let targetColumn = column;

  if (direction === "up") targetRow -= 1;
  if (direction === "down") targetRow += 1;
  if (direction === "left") targetColumn -= 1;
  if (direction === "right") targetColumn += 1;

  if (targetRow < 0 || targetColumn < 0 || targetColumn >= columns) {
    return null;
  }

  const target = getIndex(targetRow, targetColumn, columns);
  // Vertical movement keeps the exact logical column. If that column is not
  // present in an incomplete last row, movement stops at the current item.
  if (target < 0 || target >= totalItems) return null;
  if (
    (direction === "left" || direction === "right") &&
    getRow(target, columns) !== row
  ) {
    return null;
  }
  return target;
}

export function getWindowForTarget(
  targetIndex: number,
  currentWindow: VirtualGridWindow,
  config: VirtualGridWindowConfig,
): VirtualGridWindow {
  const { totalItems, columns, visibleRows, overscanRows } = config;
  assertGrid(columns);
  if (!Number.isInteger(totalItems) || totalItems < 0) {
    throw new Error("Virtual grid totalItems must be a non-negative integer");
  }
  if (!Number.isInteger(visibleRows) || visibleRows < 1) {
    throw new Error("Virtual grid visibleRows must be positive");
  }
  if (!Number.isInteger(overscanRows) || overscanRows < 0) {
    throw new Error("Virtual grid overscanRows must be non-negative");
  }
  if (totalItems === 0) return { start: 0, end: 0 };
  assertIndex(targetIndex, totalItems);

  const windowSize = visibleRows * columns;
  const maxStart = Math.max(
    0,
    Math.ceil(totalItems / columns) * columns - windowSize,
  );
  const currentStart = Math.max(
    0,
    Math.min(Math.floor(currentWindow.start / columns) * columns, maxStart),
  );
  const currentEnd = Math.min(totalItems, currentStart + windowSize);
  const targetRow = getRow(targetIndex, columns);
  const currentStartRow = getRow(currentStart, columns);
  const currentEndRow = Math.ceil(currentEnd / columns);
  const safeStartRow = currentStartRow + overscanRows;
  const safeEndRow = Math.max(safeStartRow, currentEndRow - overscanRows);

  if (targetRow >= safeStartRow && targetRow < safeEndRow) {
    return { start: currentStart, end: currentEnd };
  }

  const nextStartRow =
    targetRow < safeStartRow
      ? targetRow - overscanRows
      : targetRow - visibleRows + overscanRows;
  const nextStart = Math.max(
    0,
    Math.min(Math.floor(nextStartRow) * columns, maxStart),
  );
  return {
    start: nextStart,
    end: Math.min(totalItems, nextStart + windowSize),
  };
}
