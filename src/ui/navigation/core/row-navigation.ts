export interface HomeRowItem {
  focusId: string;
  rowIndex: number;
  itemIndex: number;
  disabled: boolean;
  hidden?: boolean;
  centerX?: number;
}

export interface HomeNavigationState {
  activeRowIndex: number;
  activeItemIndex: number;
  preferredItemIndex: number;
  preferredCenterX?: number;
}

export interface RowNavigationRegistration extends HomeRowItem {
  groupId: string;
  rowId: string;
}

export type VerticalSelectionStrategy =
  | "last-restored"
  | "same-index"
  | "nearest-horizontal"
  | "last-available"
  | "first-valid"
  | "no-target";

export interface VerticalTarget {
  item: HomeRowItem | null;
  strategy: VerticalSelectionStrategy;
  fallbackReason?: string;
}

export function getValidRowItems(
  items: readonly HomeRowItem[],
  rowIndex: number,
): HomeRowItem[] {
  return items
    .filter(
      (item) => item.rowIndex === rowIndex && !item.disabled && !item.hidden,
    )
    .sort(
      (left, right) =>
        left.itemIndex - right.itemIndex ||
        left.focusId.localeCompare(right.focusId),
    );
}

export function getHorizontalTarget(
  items: readonly HomeRowItem[],
  currentItemIndex: number,
  direction: "left" | "right",
  wrap = false,
): HomeRowItem | null {
  const validItems = getValidRowItems(
    items,
    items.find((item) => item.itemIndex === currentItemIndex)?.rowIndex ??
      items[0]?.rowIndex ??
      -1,
  );
  const currentIndex = validItems.findIndex(
    (item) => item.itemIndex === currentItemIndex,
  );
  if (currentIndex < 0 || validItems.length < 2) return null;

  let targetIndex = currentIndex + (direction === "left" ? -1 : 1);
  if (targetIndex < 0 || targetIndex >= validItems.length) {
    if (!wrap) return null;
    targetIndex = (targetIndex + validItems.length) % validItems.length;
  }
  return validItems[targetIndex] ?? null;
}

export function getNearestHorizontalItem(
  items: readonly HomeRowItem[],
  preferredItemIndex: number,
  preferredCenterX?: number,
): HomeRowItem | null {
  const validItems = items.filter((item) => !item.disabled && !item.hidden);
  if (validItems.length === 0) return null;

  return (
    [...validItems].sort((left, right) => {
      const leftCenterDistance =
        preferredCenterX === undefined || left.centerX === undefined
          ? Number.POSITIVE_INFINITY
          : Math.abs(left.centerX - preferredCenterX);
      const rightCenterDistance =
        preferredCenterX === undefined || right.centerX === undefined
          ? Number.POSITIVE_INFINITY
          : Math.abs(right.centerX - preferredCenterX);
      return (
        leftCenterDistance - rightCenterDistance ||
        Math.abs(left.itemIndex - preferredItemIndex) -
          Math.abs(right.itemIndex - preferredItemIndex) ||
        left.itemIndex - right.itemIndex ||
        left.focusId.localeCompare(right.focusId)
      );
    })[0] ?? null
  );
}

export function preservePreferredPosition(
  state: HomeNavigationState,
  item: HomeRowItem,
  preserve = true,
): HomeNavigationState {
  return {
    activeRowIndex: item.rowIndex,
    activeItemIndex: item.itemIndex,
    preferredItemIndex: preserve ? state.preferredItemIndex : item.itemIndex,
    preferredCenterX: preserve
      ? state.preferredCenterX
      : (item.centerX ?? state.preferredCenterX),
  };
}

export function getVerticalTarget(
  targetRowItems: readonly HomeRowItem[],
  state: HomeNavigationState,
  lastFocusedFocusId?: string,
): VerticalTarget {
  const validItems = targetRowItems.filter(
    (item) => !item.disabled && !item.hidden,
  );
  if (validItems.length === 0) {
    return { item: null, strategy: "no-target", fallbackReason: "empty-row" };
  }

  if (lastFocusedFocusId) {
    const restored = validItems.find(
      (item) => item.focusId === lastFocusedFocusId,
    );
    if (restored) return { item: restored, strategy: "last-restored" };
  }

  const sameIndex = validItems.find(
    (item) => item.itemIndex === state.preferredItemIndex,
  );
  if (sameIndex) return { item: sameIndex, strategy: "same-index" };

  const nearest = getNearestHorizontalItem(
    validItems,
    state.preferredItemIndex,
    state.preferredCenterX,
  );
  if (nearest) {
    const maxItemIndex = Math.max(...validItems.map((item) => item.itemIndex));
    if (
      state.preferredCenterX === undefined &&
      state.preferredItemIndex > maxItemIndex
    ) {
      return {
        item: nearest,
        strategy: "last-available",
        fallbackReason: "preferred-index-out-of-range",
      };
    }
    return {
      item: nearest,
      strategy: "nearest-horizontal",
      fallbackReason: "equivalent-item-unavailable",
    };
  }

  return {
    item: validItems[0] ?? null,
    strategy: "first-valid",
    fallbackReason: "no-secondary-candidate",
  };
}

export class NavigationRowCoordinator {
  private readonly states = new Map<string, HomeNavigationState>();
  private readonly lastFocusedByRow = new Map<string, Map<number, string>>();
  private readonly restoredGroups = new Set<string>();

  public reset(): void {
    this.states.clear();
    this.lastFocusedByRow.clear();
    this.restoredGroups.clear();
  }

  public recordFocus(
    item: RowNavigationRegistration,
    options?: { preservePreferredPosition?: boolean; restored?: boolean },
  ): void {
    const previous = this.states.get(item.groupId) ?? {
      activeRowIndex: item.rowIndex,
      activeItemIndex: item.itemIndex,
      preferredItemIndex: item.itemIndex,
      preferredCenterX: item.centerX,
    };
    this.states.set(
      item.groupId,
      preservePreferredPosition(
        previous,
        item,
        options?.preservePreferredPosition ?? false,
      ),
    );
    const rowFocus = this.lastFocusedByRow.get(item.groupId) ?? new Map();
    rowFocus.set(item.rowIndex, item.focusId);
    this.lastFocusedByRow.set(item.groupId, rowFocus);
    if (options?.restored) this.restoredGroups.add(item.groupId);
  }

  public getState(groupId: string): HomeNavigationState | undefined {
    const state = this.states.get(groupId);
    return state ? { ...state } : undefined;
  }

  public resolveVertical(
    groupId: string,
    current: RowNavigationRegistration,
    direction: "up" | "down",
    items: readonly RowNavigationRegistration[],
  ): VerticalTarget & { targetRowId?: string } {
    const state =
      this.states.get(groupId) ??
      preservePreferredPosition(
        {
          activeRowIndex: current.rowIndex,
          activeItemIndex: current.itemIndex,
          preferredItemIndex: current.itemIndex,
          preferredCenterX: current.centerX,
        },
        current,
        false,
      );
    const rows = [
      ...new Set(
        items
          .filter((item) => item.groupId === groupId)
          .map((item) => item.rowIndex),
      ),
    ].sort((left, right) => left - right);
    const currentRowPosition = rows.indexOf(current.rowIndex);
    const delta = direction === "up" ? -1 : 1;
    let targetRowPosition = currentRowPosition + delta;

    while (targetRowPosition >= 0 && targetRowPosition < rows.length) {
      const targetRowIndex = rows[targetRowPosition];
      const targetItems = getValidRowItems(items, targetRowIndex);
      if (targetItems.length > 0) {
        const rowFocus = this.lastFocusedByRow.get(groupId);
        const lastFocusedFocusId = this.restoredGroups.has(groupId)
          ? rowFocus?.get(targetRowIndex)
          : undefined;
        const target = getVerticalTarget(
          targetItems,
          state,
          lastFocusedFocusId,
        );
        this.restoredGroups.delete(groupId);
        const targetRegistration = target.item
          ? items.find((item) => item.focusId === target.item?.focusId)
          : undefined;
        return {
          ...target,
          targetRowId: targetRegistration?.rowId,
        };
      }
      targetRowPosition += delta;
    }

    return {
      item: null,
      strategy: "no-target",
      fallbackReason: "no-valid-adjacent-row",
    };
  }
}
