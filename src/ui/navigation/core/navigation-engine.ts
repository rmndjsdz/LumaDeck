import { ACTION_TO_DIRECTION, isDirectionAction } from "./navigation-actions";
import { canRestoreFocus } from "./focus-restoration";
import { FocusHistory } from "./focus-history";
import { FocusRegistry } from "./focus-registry";
import { findSpatialCandidate } from "./spatial-navigation";
import { useNavigationStore } from "../../../stores/navigation-store";
import type {
  FocusEntry,
  NavigationAction,
  NavigationDirection,
  ScopeRegistration,
} from "./navigation-types";
import type { FocusScrollManager } from "../scroll/focus-scroll-manager";
import { getColumn, getDirectionalTarget, getRow } from "./virtual-grid";

interface RegisteredScope extends Omit<ScopeRegistration, "parentScopeId"> {
  parentScopeId: string | null;
  openerFocusId?: string;
}

interface PendingGridFocusRequest {
  requestId: number;
  scopeId: string;
  groupId: string;
  targetAbsoluteIndex: number;
  targetFocusId: string;
  direction: NavigationDirection;
  column: number;
  grid: NonNullable<FocusEntry["gridNavigation"]>;
}

export class NavigationEngine {
  private readonly scopes = new Map<string, RegisteredScope>();
  private readonly pendingOpeners = new Map<string, string | undefined>();
  private readonly focusHistory = new FocusHistory();
  private readonly logicalGridIndices = new Map<string, number>();
  private readonly unregisterRegistryListener: () => void;
  private pendingGridFocus: PendingGridFocusRequest | null = null;
  private pendingFrame: number | null = null;
  private pendingTimeout: number | null = null;
  private nextGridRequestId = 0;

  public constructor(
    public readonly registry: FocusRegistry,
    private readonly scrollManager: FocusScrollManager,
  ) {
    this.unregisterRegistryListener = registry.subscribe(() => {
      this.tryCompletePendingGridFocus();
    });
  }

  public dispose(): void {
    this.unregisterRegistryListener();
    if (this.pendingFrame !== null) {
      window.cancelAnimationFrame(this.pendingFrame);
      this.pendingFrame = null;
    }
    if (this.pendingTimeout !== null) {
      window.clearTimeout(this.pendingTimeout);
      this.pendingTimeout = null;
    }
    this.pendingGridFocus = null;
  }

  public registerScope(scope: ScopeRegistration): () => void {
    const parentScopeId =
      scope.parentScopeId ?? useNavigationStore.getState().activeScopeId;
    const registered: RegisteredScope = {
      ...scope,
      parentScopeId,
      openerFocusId: this.pendingOpeners.get(scope.scopeId),
    };
    this.pendingOpeners.delete(scope.scopeId);
    this.scopes.set(scope.scopeId, registered);

    if (
      registered.activateOnMount ||
      registered.openerFocusId ||
      useNavigationStore.getState().activeScopeId === null
    ) {
      this.activateScope(scope.scopeId, registered.initialFocusId);
    }

    return () => this.unregisterScope(scope.scopeId);
  }

  public unregisterScope(scopeId: string): void {
    const scope = this.scopes.get(scopeId);
    if (!scope) return;
    const wasActive = useNavigationStore.getState().activeScopeId === scopeId;
    this.focusHistory.remember(
      scopeId,
      useNavigationStore.getState().activeFocusId,
    );
    if (scope.openerFocusId) {
      this.pendingOpeners.set(scopeId, scope.openerFocusId);
    }
    this.scopes.delete(scopeId);

    if (wasActive && scope.parentScopeId) {
      const restoreId = scope.restoreFocus ? scope.openerFocusId : undefined;
      this.activateScope(scope.parentScopeId, restoreId);
      if (restoreId) {
        useNavigationStore.getState().updateDebug({
          lastRestoredFocus: restoreId,
        });
      }
    } else if (wasActive) {
      useNavigationStore.getState().setActiveScopeId(null);
      this.setFocus(null);
    }
  }

  public prepareScopeOpen(scopeId: string, openerFocusId?: string): void {
    const activeScopeId = useNavigationStore.getState().activeScopeId;
    if (
      activeScopeId &&
      activeScopeId !== scopeId &&
      this.scopes.get(activeScopeId)?.modal
    ) {
      return;
    }
    this.pendingOpeners.set(scopeId, openerFocusId);
    if (this.scopes.has(scopeId)) this.activateScope(scopeId, openerFocusId);
  }

  public activateScope(scopeId: string, preferredFocusId?: string): boolean {
    const activeScopeId = useNavigationStore.getState().activeScopeId;
    if (
      activeScopeId &&
      activeScopeId !== scopeId &&
      this.scopes.get(activeScopeId)?.modal
    ) {
      return false;
    }
    const scope = this.scopes.get(scopeId);
    if (!scope) return false;
    if (activeScopeId && activeScopeId !== scopeId) {
      this.scrollManager.rememberScope(activeScopeId);
    }
    useNavigationStore.getState().setActiveScopeId(scopeId);
    this.syncTabStops();
    this.scrollManager.restoreScope(scopeId);

    const remembered = scope.rememberScroll
      ? this.focusHistory.get(scopeId)
      : undefined;
    const preferredInScope =
      preferredFocusId &&
      this.registry.get(preferredFocusId)?.scopeId === scopeId
        ? preferredFocusId
        : undefined;
    const focusId =
      preferredInScope ??
      (canRestoreFocus(this.registry, remembered) ? remembered : undefined) ??
      scope.initialFocusId ??
      this.registry.getScopeEntries(scopeId)[0]?.focusId;
    if (focusId) this.focus(focusId, false);
    return Boolean(focusId);
  }

  public getActiveScopeId(): string | null {
    return useNavigationStore.getState().activeScopeId;
  }

  public getActiveFocusId(): string | null {
    return useNavigationStore.getState().activeFocusId;
  }

  public focus(focusId: string, activateScope = true): boolean {
    const entry = this.registry.get(focusId);
    if (
      !entry ||
      entry.disabled ||
      entry.hidden ||
      !entry.element.isConnected
    ) {
      return false;
    }
    const activeScopeId = useNavigationStore.getState().activeScopeId;
    if (
      activeScopeId &&
      activeScopeId !== entry.scopeId &&
      this.scopes.get(activeScopeId)?.modal
    ) {
      return false;
    }
    if (activateScope && activeScopeId !== entry.scopeId) {
      return this.activateScope(entry.scopeId, focusId);
    }
    this.setFocus(entry);
    return true;
  }

  public dispatch(action: NavigationAction): boolean {
    useNavigationStore.getState().recordAction(action);
    if (isDirectionAction(action)) {
      return this.move(ACTION_TO_DIRECTION[action] ?? "down");
    }

    const activeFocusId = useNavigationStore.getState().activeFocusId;
    const activeEntry = activeFocusId
      ? this.registry.get(activeFocusId)
      : undefined;
    if (action === "confirm") {
      activeEntry?.onConfirm?.();
      return Boolean(activeEntry);
    }
    if (action === "back") return this.back();
    return (
      action === "menu" || action === "page-next" || action === "page-previous"
    );
  }

  private move(direction: "up" | "down" | "left" | "right"): boolean {
    const state = useNavigationStore.getState();
    const scopeId = state.activeScopeId;
    const pending = this.pendingGridFocus;
    const current = state.activeFocusId
      ? this.registry.get(state.activeFocusId)
      : undefined;
    if (!scopeId) return false;
    if (pending?.scopeId === scopeId) {
      return this.moveGrid(scopeId, pending.targetFocusId, direction, pending);
    }
    if (!current || current.scopeId !== scopeId) {
      return this.activateScope(scopeId);
    }

    if (current.gridNavigation) {
      return this.moveGrid(scopeId, current.focusId, direction);
    }

    const linear = current.linearNavigation;
    const isLinearDirection =
      linear &&
      ((linear.axis === "horizontal" &&
        (direction === "left" || direction === "right")) ||
        (linear.axis === "vertical" &&
          (direction === "up" || direction === "down")));
    if (isLinearDirection) {
      const entries = this.registry
        .getScopeEntries(scopeId)
        .filter(
          (entry) =>
            entry.linearNavigation?.groupId === linear.groupId &&
            entry.linearNavigation.axis === linear.axis,
        );
      const currentIndex = entries.findIndex(
        (entry) => entry.focusId === current.focusId,
      );
      if (currentIndex < 0) return false;
      const delta = direction === "left" || direction === "up" ? -1 : 1;
      let targetIndex = currentIndex + delta;
      if (targetIndex < 0 || targetIndex >= entries.length) {
        if (!linear.wrap || entries.length === 0) {
          this.recordResolution(
            direction,
            undefined,
            entries.map((entry) => entry.focusId),
            0,
          );
          return false;
        }
        targetIndex = (targetIndex + entries.length) % entries.length;
      }
      const target = entries[targetIndex];
      this.recordResolution(
        direction,
        target?.focusId,
        entries.map((entry) => entry.focusId),
        0,
      );
      return target ? this.focus(target.focusId) : false;
    }

    const override = current.navigation?.[direction];
    if (override && this.focus(override)) {
      this.recordResolution(direction, override, [], 0);
      return true;
    }

    const currentRect = this.registry.getRect(current);
    const candidates = this.registry.getScopeEntries(scopeId).map((entry) => ({
      focusId: entry.focusId,
      rect: this.registry.getRect(entry),
      priority: entry.priority,
    }));
    const resolution = findSpatialCandidate(
      currentRect,
      candidates.filter((candidate) => candidate.focusId !== current.focusId),
      direction,
    );
    this.recordResolution(
      direction,
      resolution.candidate?.focusId,
      resolution.evaluated,
      resolution.durationMs,
    );
    if (resolution.candidate) return this.focus(resolution.candidate.focusId);

    const scope = this.scopes.get(scopeId);
    if (scope?.trapFocus) {
      const entries = this.registry.getScopeEntries(scopeId);
      const currentIndex = entries.findIndex(
        (entry) => entry.focusId === current.focusId,
      );
      const delta = direction === "up" || direction === "left" ? -1 : 1;
      const fallback =
        currentIndex >= 0 && entries.length > 0
          ? entries[(currentIndex + delta + entries.length) % entries.length]
          : undefined;
      return fallback ? this.focus(fallback.focusId) : false;
    }
    return false;
  }

  private moveGrid(
    scopeId: string,
    currentFocusId: string,
    direction: "up" | "down" | "left" | "right",
    pending?: PendingGridFocusRequest,
  ): boolean {
    const current = this.registry.get(currentFocusId);
    const grid = pending?.grid ?? current?.gridNavigation;
    if (!grid) return false;
    const entries = this.registry
      .getScopeEntries(scopeId, { includeDisabled: true, includeHidden: true })
      .filter((entry) => entry.gridNavigation?.groupId === grid.groupId);
    const registryIndex = current
      ? entries.findIndex((entry) => entry.focusId === currentFocusId)
      : -1;
    if (!pending && registryIndex < 0) return false;
    if (grid.columns < 1) return false;
    const currentIndex =
      pending?.targetAbsoluteIndex ??
      this.logicalGridIndices.get(grid.groupId) ??
      grid.index ??
      registryIndex;
    const itemCount = grid.itemCount ?? entries.length;

    const column = getColumn(currentIndex, grid.columns);
    const targetIndex = getDirectionalTarget(
      currentIndex,
      direction,
      itemCount,
      grid.columns,
    );
    if (targetIndex === null) {
      this.recordResolution(direction, undefined, [], 0);
      return false;
    }

    const step =
      direction === "up"
        ? -grid.columns
        : direction === "down"
          ? grid.columns
          : direction === "left"
            ? -1
            : 1;
    const row = getRow(currentIndex, grid.columns);
    let candidateIndex = targetIndex;
    while (candidateIndex >= 0 && candidateIndex < itemCount) {
      const candidate = entries.find(
        (entry, entryIndex) =>
          (entry.gridNavigation?.index ?? entryIndex) === candidateIndex,
      );
      if (!candidate && grid.resolveFocusId && grid.onRequestIndex) {
        const focusId = grid.resolveFocusId(candidateIndex);
        this.logicalGridIndices.set(grid.groupId, candidateIndex);
        this.recordResolution(direction, focusId, [], 0);
        this.requestGridFocus(
          scopeId,
          grid,
          candidateIndex,
          focusId,
          direction,
          column,
        );
        return true;
      }
      if (candidate && !candidate.disabled && !candidate.hidden) {
        this.logicalGridIndices.set(grid.groupId, candidateIndex);
        this.updateGridDebug(grid, candidateIndex, undefined, direction);
        this.clearPendingGridFocus();
        this.recordResolution(direction, candidate.focusId, [], 0);
        return this.focus(candidate.focusId);
      }
      candidateIndex += step;
      if (candidateIndex < 0 || candidateIndex >= itemCount) break;
      if (
        (direction === "left" || direction === "right") &&
        getRow(candidateIndex, grid.columns) !== row
      ) {
        break;
      }
      if (
        (direction === "up" || direction === "down") &&
        getColumn(candidateIndex, grid.columns) !== column
      ) {
        break;
      }
    }

    this.recordResolution(direction, undefined, [], 0);
    return false;
  }

  public getActiveAbsoluteIndex(groupId?: string): number | null {
    if (groupId) return this.logicalGridIndices.get(groupId) ?? null;
    const activeFocusId = useNavigationStore.getState().activeFocusId;
    const entry = activeFocusId ? this.registry.get(activeFocusId) : undefined;
    const activeGroupId = entry?.gridNavigation?.groupId;
    return activeGroupId
      ? (this.logicalGridIndices.get(activeGroupId) ??
          entry?.gridNavigation?.index ??
          null)
      : null;
  }

  private requestGridFocus(
    scopeId: string,
    grid: NonNullable<FocusEntry["gridNavigation"]>,
    targetAbsoluteIndex: number,
    targetFocusId: string,
    direction: NavigationDirection,
    column: number,
  ): void {
    this.clearPendingGridFocus();
    const request: PendingGridFocusRequest = {
      requestId: ++this.nextGridRequestId,
      scopeId,
      groupId: grid.groupId,
      targetAbsoluteIndex,
      targetFocusId,
      direction,
      column,
      grid,
    };
    this.pendingGridFocus = request;
    this.updateGridDebug(grid, targetAbsoluteIndex, request, direction);
    grid.onRequestIndex?.(targetAbsoluteIndex);
    this.tryCompletePendingGridFocus();
  }

  private tryCompletePendingGridFocus(): void {
    const pending = this.pendingGridFocus;
    if (!pending || this.pendingFrame !== null) return;
    const entry = this.registry.get(pending.targetFocusId);
    if (
      !entry ||
      entry.scopeId !== pending.scopeId ||
      entry.gridNavigation?.groupId !== pending.groupId ||
      entry.gridNavigation.index !== pending.targetAbsoluteIndex ||
      entry.disabled ||
      entry.hidden ||
      !entry.element.isConnected
    ) {
      return;
    }

    const requestId = pending.requestId;
    this.pendingFrame = window.requestAnimationFrame(() => {
      this.pendingFrame = null;
      const current = this.pendingGridFocus;
      if (!current || current.requestId !== requestId) return;
      const target = this.registry.get(current.targetFocusId);
      if (!target || !target.element.isConnected) return;
      this.pendingGridFocus = null;
      if (this.pendingTimeout !== null) {
        window.clearTimeout(this.pendingTimeout);
        this.pendingTimeout = null;
      }
      this.updateGridDebug(current.grid, current.targetAbsoluteIndex);
      this.focus(target.focusId);
    });

    this.pendingTimeout = window.setTimeout(() => {
      const current = this.pendingGridFocus;
      if (!current || current.requestId !== requestId) return;
      this.pendingTimeout = null;
      if (import.meta.env.DEV) {
        console.warn(
          `[navigation] virtual focus request ${requestId} did not materialize`,
        );
      }
      useNavigationStore.getState().updateDebug({
        fallbackReason: "virtual-focus-timeout",
      });
    }, 1000);
  }

  private clearPendingGridFocus(): void {
    this.pendingGridFocus = null;
    if (this.pendingFrame !== null) {
      window.cancelAnimationFrame(this.pendingFrame);
      this.pendingFrame = null;
    }
    if (this.pendingTimeout !== null) {
      window.clearTimeout(this.pendingTimeout);
      this.pendingTimeout = null;
    }
    useNavigationStore.getState().updateDebug({
      pendingFocusId: undefined,
      pendingRequestId: undefined,
    });
  }

  private updateGridDebug(
    grid: NonNullable<FocusEntry["gridNavigation"]>,
    activeAbsoluteIndex: number,
    pending?: PendingGridFocusRequest,
    direction?: NavigationDirection,
  ): void {
    const activeRow = getRow(activeAbsoluteIndex, grid.columns);
    const activeColumn = getColumn(activeAbsoluteIndex, grid.columns);
    const targetAbsoluteIndex = pending?.targetAbsoluteIndex;
    useNavigationStore.getState().updateDebug({
      activeAbsoluteIndex,
      activeRow,
      activeColumn,
      targetAbsoluteIndex,
      targetRow:
        targetAbsoluteIndex === undefined
          ? undefined
          : getRow(targetAbsoluteIndex, grid.columns),
      targetColumn:
        targetAbsoluteIndex === undefined
          ? undefined
          : getColumn(targetAbsoluteIndex, grid.columns),
      pendingFocusId: pending?.targetFocusId,
      pendingRequestId: pending?.requestId,
      requestedDirection: direction,
    });
  }

  private back(): boolean {
    const scopeId = useNavigationStore.getState().activeScopeId;
    const scope = scopeId ? this.scopes.get(scopeId) : undefined;
    if (scope?.onBack?.() === true) return true;
    return false;
  }

  private setFocus(entry: FocusEntry | null): void {
    const state = useNavigationStore.getState();
    if (state.activeFocusId === entry?.focusId) return;
    if (entry?.gridNavigation?.index !== undefined) {
      this.logicalGridIndices.set(
        entry.gridNavigation.groupId,
        entry.gridNavigation.index,
      );
      this.updateGridDebug(entry.gridNavigation, entry.gridNavigation.index);
    }
    const previousEntry = state.activeFocusId
      ? this.registry.get(state.activeFocusId)
      : undefined;
    if (previousEntry) {
      previousEntry.element.dataset.active = "false";
      previousEntry.element.tabIndex = -1;
      previousEntry.onBlur?.();
    }
    useNavigationStore.getState().setActiveFocusId(entry?.focusId ?? null);
    this.syncTabStops();
    if (entry) {
      entry.element.dataset.active = "true";
      entry.element.tabIndex = 0;
    }
    entry?.onFocus?.();
    if (entry) {
      entry.element.focus({ preventScroll: true });
      const scrollResult = this.scrollManager.ensureVisible(
        entry.element,
        entry.focusId,
      );
      if (scrollResult.scrolled) {
        useNavigationStore.getState().updateDebug({
          lastScroll: scrollResult.focusId,
          scrollAuthority: "focus",
        });
      }
    }
  }

  public handleTab(shiftKey: boolean): boolean {
    const state = useNavigationStore.getState();
    const scope = state.activeScopeId
      ? this.scopes.get(state.activeScopeId)
      : undefined;
    if (!scope?.trapFocus || !state.activeScopeId) return false;
    const entries = this.registry.getScopeEntries(state.activeScopeId);
    const currentIndex = entries.findIndex(
      (entry) => entry.focusId === state.activeFocusId,
    );
    if (currentIndex < 0 || entries.length === 0) return false;
    const delta = shiftKey ? -1 : 1;
    const target =
      entries[(currentIndex + delta + entries.length) % entries.length];
    return target ? this.focus(target.focusId) : false;
  }

  private syncTabStops(): void {
    const state = useNavigationStore.getState();
    for (const entry of this.registry.getEntries()) {
      if (!entry.element.isConnected) continue;
      const active =
        entry.scopeId === state.activeScopeId &&
        entry.focusId === state.activeFocusId;
      entry.element.tabIndex = active ? 0 : -1;
      entry.element.dataset.scopePaused =
        entry.scopeId === state.activeScopeId ? "false" : "true";
    }
  }

  private recordResolution(
    direction: "up" | "down" | "left" | "right",
    candidate: string | undefined,
    evaluated: string[],
    durationMs: number,
  ): void {
    useNavigationStore.getState().updateDebug({
      requestedDirection: direction,
      resolvedCandidate: candidate,
      evaluatedCandidates: evaluated,
      resolutionTimeMs: durationMs,
    });
  }
}
