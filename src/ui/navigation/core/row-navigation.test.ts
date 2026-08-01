import { describe, expect, it } from "vitest";

import {
  NavigationRowCoordinator,
  getHorizontalTarget,
  getNearestHorizontalItem,
  getValidRowItems,
  getVerticalTarget,
  preservePreferredPosition,
  type HomeNavigationState,
  type HomeRowItem,
  type RowNavigationRegistration,
} from "./row-navigation";

function item(
  rowIndex: number,
  itemIndex: number,
  options?: Partial<HomeRowItem>,
): HomeRowItem {
  return {
    focusId: `row-${rowIndex}-${itemIndex}`,
    rowIndex,
    itemIndex,
    disabled: false,
    ...options,
  };
}

function registration(
  rowIndex: number,
  itemIndex: number,
  options?: Partial<RowNavigationRegistration>,
): RowNavigationRegistration {
  return {
    ...item(rowIndex, itemIndex),
    groupId: "home",
    rowId: `row-${rowIndex}`,
    ...options,
  };
}

describe("row navigation model", () => {
  it("keeps horizontal movement inside the current row", () => {
    const items = [item(0, 0), item(0, 1), item(0, 2)];

    expect(getHorizontalTarget(items, 0, "left")).toBeNull();
    expect(getHorizontalTarget(items, 1, "right")?.itemIndex).toBe(2);
    expect(getHorizontalTarget(items, 2, "right", true)?.itemIndex).toBe(0);
  });

  it("uses the equivalent item before geometry", () => {
    const state: HomeNavigationState = {
      activeRowIndex: 0,
      activeItemIndex: 2,
      preferredItemIndex: 2,
      preferredCenterX: 250,
    };
    const target = getVerticalTarget(
      [item(1, 0, { centerX: 250 }), item(1, 2, { centerX: 50 })],
      state,
    );

    expect(target.item?.itemIndex).toBe(2);
    expect(target.strategy).toBe("same-index");
  });

  it("skips disabled equivalents and chooses the nearest horizontal item", () => {
    const state: HomeNavigationState = {
      activeRowIndex: 0,
      activeItemIndex: 2,
      preferredItemIndex: 2,
      preferredCenterX: 195,
    };
    const target = getVerticalTarget(
      [
        item(1, 1, { centerX: 100 }),
        item(1, 2, { centerX: 205, disabled: true }),
        item(1, 3, { centerX: 300 }),
      ],
      state,
    );

    expect(target.item?.itemIndex).toBe(1);
    expect(target.strategy).toBe("nearest-horizontal");
    expect(getValidRowItems(target.item ? [target.item] : [], 1)).toHaveLength(
      1,
    );
  });

  it("preserves the preferred index when a short row clamps the effective target", () => {
    const state: HomeNavigationState = {
      activeRowIndex: 0,
      activeItemIndex: 4,
      preferredItemIndex: 4,
    };
    const target = getVerticalTarget(
      [item(1, 0), item(1, 1), item(1, 2)],
      state,
    );
    const nextState = preservePreferredPosition(state, target.item!, true);

    expect(target.item?.itemIndex).toBe(2);
    expect(nextState.activeItemIndex).toBe(2);
    expect(nextState.preferredItemIndex).toBe(4);
  });

  it("restores a row's last focus only after explicit restoration", () => {
    const coordinator = new NavigationRowCoordinator();
    const rows = [registration(0, 1), registration(1, 0), registration(1, 3)];
    coordinator.recordFocus(rows[2]!);
    coordinator.recordFocus(rows[0]!);
    coordinator.recordFocus(rows[0]!, { restored: true });

    const target = coordinator.resolveVertical("home", rows[0]!, "down", rows);

    expect(target.item?.itemIndex).toBe(3);
    expect(target.strategy).toBe("last-restored");
  });

  it("uses the nearest center as a deterministic secondary policy", () => {
    const target = getNearestHorizontalItem(
      [item(1, 0, { centerX: 100 }), item(1, 2, { centerX: 220 })],
      1,
      210,
    );

    expect(target?.itemIndex).toBe(2);
  });
});
