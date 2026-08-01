import { describe, expect, it } from "vitest";

import {
  getColumn,
  getDirectionalTarget,
  getIndex,
  getRow,
  getWindowForTarget,
} from "./virtual-grid";

describe("virtual grid model", () => {
  it("maps absolute indexes to stable rows and columns", () => {
    expect(getRow(12, 5)).toBe(2);
    expect(getColumn(12, 5)).toBe(2);
    expect(getIndex(2, 2, 5)).toBe(12);
  });

  it("keeps the logical column for vertical movement", () => {
    for (const column of [0, 1, 2, 3, 4]) {
      expect(getDirectionalTarget(column, "down", 200, 5)).toBe(column + 5);
      expect(getDirectionalTarget(column + 5, "up", 200, 5)).toBe(column);
    }
    expect(getDirectionalTarget(197, "down", 198, 5)).toBeNull();
    expect(getDirectionalTarget(197, "up", 198, 5)).toBe(192);
  });

  it("moves horizontal targets only within the current logical row", () => {
    expect(getDirectionalTarget(4, "right", 200, 5)).toBeNull();
    expect(getDirectionalTarget(5, "left", 200, 5)).toBeNull();
    expect(getDirectionalTarget(7, "right", 200, 5)).toBe(8);
  });

  it("shifts a virtual window by complete rows without centering the target", () => {
    const current = { start: 0, end: 60 };
    const config = {
      totalItems: 200,
      columns: 5,
      visibleRows: 12,
      overscanRows: 2,
    };

    expect(getWindowForTarget(62, current, config)).toEqual({
      start: 10,
      end: 70,
    });
    expect(getWindowForTarget(57, current, config)).toEqual({
      start: 5,
      end: 65,
    });
    expect(getWindowForTarget(2, { start: 40, end: 100 }, config)).toEqual({
      start: 0,
      end: 60,
    });
  });
});
