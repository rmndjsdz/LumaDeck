import { ACTION_TO_DIRECTION, isDirectionAction } from "./navigation-actions";
import { FocusHistory } from "./focus-history";
import { FocusRegistry } from "./focus-registry";
import { findSpatialCandidate } from "./spatial-navigation";
import { useNavigationStore } from "../../../stores/navigation-store";
import type {
  FocusEntry,
  NavigationAction,
  NavigationDirection,
  ScopeRegistration,
  ScopeLifecycleState,
} from "./navigation-types";
import type { FocusScrollManager } from "../scroll/focus-scroll-manager";
import { getColumn, getDirectionalTarget, getRow } from "./virtual-grid";
import {
  markPerformance,
  measurePerformance,
} from "../../performance/performance-marks";
import {
  NavigationRowCoordinator,
  type HomeRowItem,
  type RowNavigationRegistration,
} from "./row-navigation";
import {
  NavigationLevelCoordinator,
  type NavigationRegionEntry,
} from "./navigation-hierarchy";

interface RegisteredScope extends Omit<ScopeRegistration, "parentScopeId"> {
  parentScopeId: string | null;
  openerFocusId?: string;
  lifecycleState: ScopeLifecycleState;
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

interface PendingScopeActivation {
  requestId: number;
  scopeId: string;
  preferredFocusId?: string;
}

interface FocusChangeOptions {
  preservePreferredPosition?: boolean;
  restored?: boolean;
}

interface PendingRegionFocusRequest {
  requestId: number;
  scopeId: string;
  regionId: string;
  targetFocusId: string;
  direction: NavigationDirection;
}

export class NavigationEngine {
  private readonly scopes = new Map<string, RegisteredScope>();
  private readonly pendingOpeners = new Map<string, string | undefined>();
  private readonly focusHistory = new FocusHistory();
  private readonly logicalGridIndices = new Map<string, number>();
  private readonly rowNavigation = new NavigationRowCoordinator();
  private readonly hierarchyNavigation = new NavigationLevelCoordinator();
  private readonly unregisterRegistryListener: () => void;
  private readonly scopeWatchdogFrames = new Map<string, number>();
  private readonly scopeWatchdogWarnings = new Set<string>();
  private pendingGridFocus: PendingGridFocusRequest | null = null;
  private pendingRegionFocus: PendingRegionFocusRequest | null = null;
  private pendingScopeActivation: PendingScopeActivation | null = null;
  private pendingFrame: number | null = null;
  private pendingTimeout: number | null = null;
  private focusRetryFrame: number | null = null;
  private nextGridRequestId = 0;
  private nextScopeRequestId = 0;
  private nextRegionRequestId = 0;

  public constructor(
    public readonly registry: FocusRegistry,
    private readonly scrollManager: FocusScrollManager,
  ) {
    this.unregisterRegistryListener = registry.subscribe(() => {
      this.tryCompletePendingGridFocus();
      this.tryCompletePendingRegionFocus();
      this.tryCompletePendingScopeActivation();
      this.runScopeWatchdog();
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
    if (this.focusRetryFrame !== null) {
      window.cancelAnimationFrame(this.focusRetryFrame);
      this.focusRetryFrame = null;
    }
    for (const frame of this.scopeWatchdogFrames.values()) {
      window.cancelAnimationFrame(frame);
    }
    this.scopeWatchdogFrames.clear();
    this.scopeWatchdogWarnings.clear();
    this.pendingGridFocus = null;
    this.clearPendingRegionFocus();
    this.pendingScopeActivation = null;
    this.rowNavigation.reset();
    this.hierarchyNavigation.reset();
  }

  public registerScope(scope: ScopeRegistration): () => void {
    const parentScopeId =
      scope.parentScopeId ?? useNavigationStore.getState().activeScopeId;
    const registered: RegisteredScope = {
      ...scope,
      parentScopeId,
      openerFocusId: this.pendingOpeners.get(scope.scopeId),
      lifecycleState: "mounting",
    };
    this.pendingOpeners.delete(scope.scopeId);
    this.scopes.set(scope.scopeId, registered);

    if (
      registered.activateOnMount ||
      registered.openerFocusId ||
      useNavigationStore.getState().activeScopeId === null
    ) {
      this.requestScopeActivation(scope.scopeId);
    }

    return () => this.unregisterScope(scope.scopeId);
  }

  public unregisterScope(scopeId: string): void {
    const scope = this.scopes.get(scopeId);
    if (!scope) return;
    this.cancelPendingScopeActivation(scopeId);
    scope.lifecycleState = "unmounting";
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
      useNavigationStore.getState().setActiveScopeId(null);
      this.setFocus(null);
      this.requestScopeActivation(scope.parentScopeId, restoreId);
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
    this.cancelPendingVirtualFocus("scope-open");
    this.pendingOpeners.set(scopeId, openerFocusId);
    if (this.scopes.has(scopeId)) this.requestScopeActivation(scopeId);
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
    return this.requestScopeActivation(scopeId, preferredFocusId);
  }

  public getScopeLifecycleState(
    scopeId: string,
  ): ScopeLifecycleState | undefined {
    return this.scopes.get(scopeId)?.lifecycleState;
  }

  public cancelPendingVirtualFocus(reason = "canceled"): void {
    const canceledRequestId = this.pendingGridFocus?.requestId;
    this.clearPendingGridFocus();
    if (canceledRequestId !== undefined) {
      useNavigationStore.getState().updateDebug({
        canceledLibraryRequestId: canceledRequestId,
        fallbackReason: `virtual-focus-${reason}`,
      });
    }
  }

  private requestScopeActivation(
    scopeId: string,
    preferredFocusId?: string,
  ): boolean {
    const scope = this.scopes.get(scopeId);
    if (!scope) return false;
    this.cancelPendingScopeActivation();
    const request: PendingScopeActivation = {
      requestId: ++this.nextScopeRequestId,
      scopeId,
      preferredFocusId,
    };
    this.pendingScopeActivation = request;
    scope.lifecycleState = "mounting";
    useNavigationStore.getState().updateDebug({
      pendingScopeActivationRequestId: request.requestId,
      requestedInitialFocusId: scope.initialFocusId,
      scopeLifecycleState: scope.lifecycleState,
    });
    return this.tryCompletePendingScopeActivation();
  }

  private tryCompletePendingScopeActivation(): boolean {
    const pending = this.pendingScopeActivation;
    if (!pending) return false;
    const scope = this.scopes.get(pending.scopeId);
    if (!scope) {
      this.cancelPendingScopeActivation(pending.scopeId);
      return false;
    }
    const activeScopeId = useNavigationStore.getState().activeScopeId;
    if (
      activeScopeId &&
      activeScopeId !== pending.scopeId &&
      this.scopes.get(activeScopeId)?.modal
    ) {
      return false;
    }

    const remembered = scope.rememberScroll
      ? this.focusHistory.get(pending.scopeId)
      : undefined;
    const preferredInScope =
      pending.preferredFocusId &&
      this.isValidFocusId(pending.preferredFocusId, pending.scopeId)
        ? pending.preferredFocusId
        : undefined;
    const rememberedInScope =
      remembered && this.isValidFocusId(remembered, pending.scopeId)
        ? remembered
        : undefined;
    const initialFocus =
      scope.initialFocusId &&
      this.isValidFocusId(scope.initialFocusId, pending.scopeId)
        ? scope.initialFocusId
        : undefined;
    const focusId =
      preferredInScope ??
      rememberedInScope ??
      initialFocus ??
      (pending.preferredFocusId
        ? undefined
        : this.registry.getScopeEntries(pending.scopeId)[0]?.focusId);

    if (!focusId) {
      scope.lifecycleState = "waiting-for-focusable";
      this.updateScopeDebug(scope, pending.requestId);
      return false;
    }

    scope.lifecycleState = "activating";
    this.updateScopeDebug(scope, pending.requestId);
    if (activeScopeId && activeScopeId !== pending.scopeId) {
      const previousScope = this.scopes.get(activeScopeId);
      if (previousScope) previousScope.lifecycleState = "suspended";
      this.scrollManager.rememberScope(activeScopeId);
    }
    useNavigationStore.getState().setActiveScopeId(pending.scopeId);
    this.syncTabStops();
    this.scrollManager.restoreScope(pending.scopeId);
    const entry = this.registry.get(focusId);
    if (!entry || !this.isValidEntry(entry, pending.scopeId)) {
      scope.lifecycleState = "waiting-for-focusable";
      this.updateScopeDebug(scope, pending.requestId);
      return false;
    }

    this.pendingScopeActivation = null;
    scope.lifecycleState = "active";
    useNavigationStore.getState().updateDebug({
      pendingScopeActivationRequestId: undefined,
      scopeLifecycleState: scope.lifecycleState,
    });
    this.setFocus(entry, { restored: Boolean(pending.preferredFocusId) });
    this.updateScopeDebug(scope);
    return useNavigationStore.getState().activeFocusId === focusId;
  }

  private cancelPendingScopeActivation(scopeId?: string): void {
    const pending = this.pendingScopeActivation;
    if (!pending || (scopeId && pending.scopeId !== scopeId)) return;
    const scope = this.scopes.get(pending.scopeId);
    if (scope && scope.lifecycleState !== "unmounting") {
      scope.lifecycleState = "suspended";
    }
    this.pendingScopeActivation = null;
    useNavigationStore.getState().updateDebug({
      pendingScopeActivationRequestId: undefined,
    });
  }

  private isValidFocusId(focusId: string, scopeId: string): boolean {
    const entry = this.registry.get(focusId);
    return Boolean(entry && this.isValidEntry(entry, scopeId));
  }

  private isValidEntry(entry: FocusEntry, scopeId: string): boolean {
    return (
      entry.scopeId === scopeId &&
      !entry.disabled &&
      !entry.hidden &&
      entry.element.isConnected
    );
  }

  public getActiveScopeId(): string | null {
    return useNavigationStore.getState().activeScopeId;
  }

  public getActiveFocusId(): string | null {
    return useNavigationStore.getState().activeFocusId;
  }

  public focus(
    focusId: string,
    activateScope = true,
    options?: FocusChangeOptions,
  ): boolean {
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
    this.setFocus(entry, options);
    return true;
  }

  public dispatch(action: NavigationAction): boolean {
    useNavigationStore.getState().recordAction(action);
    if (this.isInputBlockedForPendingScope()) return false;
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
    const activeScope = this.scopes.get(scopeId);
    if (activeScope?.lifecycleState !== "active") return false;
    if (pending?.scopeId === scopeId) {
      return this.moveGrid(scopeId, pending.targetFocusId, direction, pending);
    }
    if (this.pendingRegionFocus?.scopeId === scopeId) return true;
    if (!current || current.scopeId !== scopeId) {
      return this.activateScope(scopeId);
    }

    if (current.navigationRegion && direction === "down") {
      return this.moveToChildRegion(scopeId, current);
    }

    if (
      current.navigationRegion &&
      direction === "up" &&
      current.navigationRegion.parentRegionId &&
      !current.rowNavigation &&
      !current.gridNavigation
    ) {
      return this.moveToParentRegion(scopeId, current);
    }
    if (
      current.navigationRegion &&
      direction === "up" &&
      !current.rowNavigation &&
      !current.gridNavigation
    ) {
      return false;
    }

    if (current.rowNavigation && (direction === "up" || direction === "down")) {
      return this.moveRowVertically(scopeId, current, direction);
    }

    if (current.gridNavigation) {
      if (
        current.navigationRegion &&
        direction === "up" &&
        this.isAtGridTop(current)
      ) {
        return this.moveToParentRegion(scopeId, current);
      }
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

  private moveRowVertically(
    scopeId: string,
    current: FocusEntry,
    direction: "up" | "down",
  ): boolean {
    const currentRow = current.rowNavigation;
    if (!currentRow) return false;
    const currentRect = this.registry.getRect(current);
    const rowItems = this.registry
      .getScopeEntries(scopeId, {
        includeDisabled: true,
        includeHidden: true,
      })
      .flatMap((entry) => {
        if (!entry.rowNavigation) return [];
        const rect = this.registry.getRect(entry);
        return [
          {
            ...entry.rowNavigation,
            disabled: Boolean(entry.disabled),
            hidden: Boolean(entry.hidden),
            centerX: rect.left + rect.width / 2,
            focusId: entry.focusId,
          } satisfies RowNavigationRegistration,
        ];
      });
    const target = this.rowNavigation.resolveVertical(
      currentRow.groupId,
      {
        ...currentRow,
        disabled: Boolean(current.disabled),
        hidden: Boolean(current.hidden),
        centerX: currentRect.left + currentRect.width / 2,
        focusId: current.focusId,
      },
      direction,
      rowItems,
    );
    const targetItems = rowItems.filter(
      (item) => item.rowIndex === target.item?.rowIndex,
    );
    this.updateRowDebug(currentRow, target, targetItems);
    this.recordResolution(
      direction,
      target.item?.focusId,
      targetItems.map((item) => item.focusId),
      0,
    );
    if (!target.item) {
      return direction === "up" && current.navigationRegion
        ? this.moveToParentRegion(scopeId, current)
        : false;
    }
    return this.focus(target.item.focusId, true, {
      preservePreferredPosition: currentRow.preserveHorizontalIntent,
    });
  }

  public getLastFocusedFocusId(regionId: string): string | null {
    return this.hierarchyNavigation.getLastFocusedFocusId(regionId);
  }

  public getNavigationHierarchySnapshot() {
    return this.hierarchyNavigation.getSnapshot();
  }

  public cancelPendingHierarchyFocus(): void {
    this.clearPendingRegionFocus();
  }

  private getRegionEntries(scopeId: string): NavigationRegionEntry[] {
    return this.registry
      .getScopeEntries(scopeId, {
        includeDisabled: true,
        includeHidden: true,
      })
      .flatMap((entry) => {
        const region = entry.navigationRegion;
        if (!region) return [];
        return [
          {
            ...region,
            focusId: entry.focusId,
            disabled: entry.disabled,
            hidden: entry.hidden,
          },
        ];
      });
  }

  private moveToChildRegion(scopeId: string, current: FocusEntry): boolean {
    const region = current.navigationRegion;
    if (!region?.childRegionId) return false;
    const entries = this.getRegionEntries(scopeId);
    const resolvedFocusId = this.hierarchyNavigation.resolveChild(
      region,
      entries,
    );
    const childEntries = entries.filter(
      (entry) => entry.regionId === region.childRegionId,
    );
    const hasGridMaterializer = this.registry
      .getScopeEntries(scopeId, {
        includeDisabled: true,
        includeHidden: true,
      })
      .some(
        (entry) =>
          entry.navigationRegion?.regionId === region.childRegionId &&
          entry.gridNavigation?.onRequestIndex,
      );
    const rememberedFocusId = region.childRegionId
      ? this.hierarchyNavigation.getLastFocusedFocusId(region.childRegionId)
      : null;
    const targetFocusId =
      hasGridMaterializer &&
      rememberedFocusId &&
      !childEntries.some((entry) => entry.focusId === rememberedFocusId)
        ? rememberedFocusId
        : hasGridMaterializer &&
            region.entryFocusId &&
            !childEntries.some((entry) => entry.focusId === region.entryFocusId)
          ? region.entryFocusId
          : resolvedFocusId;
    this.updateHierarchyDebug(
      current,
      targetFocusId ?? undefined,
      "down",
      "main-to-content",
      "last-focus-or-entry",
    );
    if (!targetFocusId) return false;

    const target = this.registry.get(targetFocusId);
    if (target && this.isValidEntry(target, scopeId)) {
      this.clearPendingRegionFocus();
      return this.focus(targetFocusId);
    }

    return this.requestRegionFocus(
      scopeId,
      region.childRegionId,
      targetFocusId,
      "down",
    );
  }

  private moveToParentRegion(scopeId: string, current: FocusEntry): boolean {
    const region = current.navigationRegion;
    if (!region) return false;
    const entries = this.getRegionEntries(scopeId);
    const targetFocusId = this.hierarchyNavigation.resolveParent(
      region,
      entries,
    );
    this.updateHierarchyDebug(
      current,
      targetFocusId ?? undefined,
      "up",
      targetFocusId ? "content-to-main" : "no-parent-region",
      "explicit-exit-or-parent-focus",
    );
    if (!targetFocusId) {
      if (import.meta.env.DEV) {
        console.warn(
          `[navigation] region ${region.regionId} has no valid parent focus`,
        );
      }
      return false;
    }
    this.clearPendingRegionFocus();
    return this.focus(targetFocusId);
  }

  private isAtGridTop(entry: FocusEntry): boolean {
    const grid = entry.gridNavigation;
    if (!grid) return false;
    const index =
      grid.index ?? this.logicalGridIndices.get(grid.groupId) ?? undefined;
    return index !== undefined && getRow(index, grid.columns) === 0;
  }

  private requestRegionFocus(
    scopeId: string,
    regionId: string,
    targetFocusId: string,
    direction: NavigationDirection,
  ): boolean {
    const entries = this.registry.getScopeEntries(scopeId, {
      includeDisabled: true,
      includeHidden: true,
    });
    const targetIndex =
      this.hierarchyNavigation.getPreferredItemIndex(regionId);
    const materializer = entries.find(
      (entry) =>
        entry.navigationRegion?.regionId === regionId &&
        entry.gridNavigation &&
        entry.gridNavigation.onRequestIndex,
    );
    const grid = materializer?.gridNavigation;
    if (!grid?.onRequestIndex) return false;
    const index = targetIndex ?? 0;
    const resolvedTargetFocusId = grid.resolveFocusId?.(index) ?? targetFocusId;
    this.clearPendingRegionFocus();
    this.pendingRegionFocus = {
      requestId: ++this.nextRegionRequestId,
      scopeId,
      regionId,
      targetFocusId: resolvedTargetFocusId,
      direction,
    };
    useNavigationStore.getState().updateDebug({
      pendingFocusId: resolvedTargetFocusId,
      fallbackReason: "hierarchy-region-materialization",
    });
    grid.onRequestIndex(index);
    this.tryCompletePendingRegionFocus();
    return true;
  }

  private tryCompletePendingRegionFocus(): void {
    const pending = this.pendingRegionFocus;
    if (!pending) return;
    const entry = this.registry.get(pending.targetFocusId);
    if (
      !entry ||
      entry.scopeId !== pending.scopeId ||
      entry.navigationRegion?.regionId !== pending.regionId ||
      !this.isValidEntry(entry, pending.scopeId)
    ) {
      return;
    }
    this.pendingRegionFocus = null;
    useNavigationStore.getState().updateDebug({
      pendingFocusId: undefined,
      hierarchyRestoreReason: "materialized-region-focus",
    });
    this.focus(entry.focusId);
  }

  private clearPendingRegionFocus(): void {
    this.pendingRegionFocus = null;
    useNavigationStore.getState().updateDebug({
      pendingFocusId: undefined,
    });
  }

  private isInputBlockedForPendingScope(): boolean {
    const pending = this.pendingScopeActivation;
    if (!pending) return false;
    return useNavigationStore.getState().activeScopeId !== pending.scopeId;
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

  private runScopeWatchdog(): void {
    const state = useNavigationStore.getState();
    if (!state.activeScopeId) return;
    if (this.pendingGridFocus?.scopeId === state.activeScopeId) return;
    if (
      this.pendingScopeActivation &&
      this.pendingScopeActivation.scopeId !== state.activeScopeId
    ) {
      return;
    }
    if (this.pendingOpeners.size > 0) return;
    const scope = this.scopes.get(state.activeScopeId);
    if (!scope || scope.lifecycleState !== "active") return;
    const entries = this.registry.getScopeEntries(state.activeScopeId);
    const activeEntry = state.activeFocusId
      ? this.registry.get(state.activeFocusId)
      : undefined;
    const activeFocusValid = Boolean(
      activeEntry && this.isValidEntry(activeEntry, state.activeScopeId),
    );
    this.updateScopeDebug(scope);
    if (entries.length > 0 && activeFocusValid) {
      this.scopeWatchdogWarnings.delete(scope.scopeId);
      const frame = this.scopeWatchdogFrames.get(scope.scopeId);
      if (frame !== undefined) {
        window.cancelAnimationFrame(frame);
        this.scopeWatchdogFrames.delete(scope.scopeId);
      }
      return;
    }

    if (this.scopeWatchdogFrames.has(scope.scopeId)) return;
    const frame = window.requestAnimationFrame(() => {
      this.scopeWatchdogFrames.delete(scope.scopeId);
      const currentState = useNavigationStore.getState();
      if (
        currentState.activeScopeId !== scope.scopeId ||
        scope.lifecycleState !== "active"
      ) {
        return;
      }
      const currentEntries = this.registry.getScopeEntries(scope.scopeId);
      const recoveryId =
        scope.initialFocusId &&
        this.isValidFocusId(scope.initialFocusId, scope.scopeId)
          ? scope.initialFocusId
          : currentEntries[0]?.focusId;
      if (recoveryId) {
        this.setFocus(this.registry.get(recoveryId) ?? null);
        this.updateScopeDebug(scope);
        return;
      }
      if (
        import.meta.env.DEV &&
        !this.scopeWatchdogWarnings.has(scope.scopeId)
      ) {
        this.scopeWatchdogWarnings.add(scope.scopeId);
        console.warn(
          `[navigation] active scope ${scope.scopeId} has invalid focus`,
        );
      }
      scope.lifecycleState = "waiting-for-focusable";
      useNavigationStore.getState().setActiveFocusId(null);
      useNavigationStore.getState().updateDebug({
        scopeLifecycleState: scope.lifecycleState,
        lastFocusFailureReason: "scope-watchdog-no-focusable",
      });
      this.pendingScopeActivation = {
        requestId: ++this.nextScopeRequestId,
        scopeId: scope.scopeId,
      };
      useNavigationStore.getState().updateDebug({
        pendingScopeActivationRequestId: this.pendingScopeActivation.requestId,
      });
    });
    this.scopeWatchdogFrames.set(scope.scopeId, frame);
  }

  private updateScopeDebug(scope: RegisteredScope, requestId?: number): void {
    const state = useNavigationStore.getState();
    const entries = this.registry.getScopeEntries(scope.scopeId);
    const activeEntry = state.activeFocusId
      ? this.registry.get(state.activeFocusId)
      : undefined;
    const activeFocusValid = Boolean(
      activeEntry && this.isValidEntry(activeEntry, scope.scopeId),
    );
    useNavigationStore.getState().updateDebug({
      scopeLifecycleState: scope.lifecycleState,
      requestedInitialFocusId: scope.initialFocusId,
      registeredActiveScopeFocusables:
        state.activeScopeId === scope.scopeId ? entries.length : undefined,
      activeFocusValid:
        state.activeScopeId === scope.scopeId ? activeFocusValid : undefined,
      domActiveElementFocusId:
        document.activeElement?.getAttribute("data-focus-id") ?? undefined,
      pendingScopeActivationRequestId: requestId,
    });
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
    if (this.pendingGridFocus) this.nextGridRequestId += 1;
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

  private updateRowDebug(
    current: NonNullable<FocusEntry["rowNavigation"]>,
    target: {
      item: HomeRowItem | null;
      strategy: string;
      fallbackReason?: string;
      targetRowId?: string;
    },
    targetItems: readonly RowNavigationRegistration[],
  ): void {
    const state = this.rowNavigation.getState(current.groupId);
    useNavigationStore.getState().updateDebug({
      activeHomeRowId: current.rowId,
      activeHomeRowIndex: state?.activeRowIndex ?? current.rowIndex,
      activeHomeItemIndex: state?.activeItemIndex ?? current.itemIndex,
      preferredHomeItemIndex: state?.preferredItemIndex ?? current.itemIndex,
      preferredHomeCenterX: state?.preferredCenterX,
      targetHomeRowId: target.targetRowId,
      targetHomeItemIndex: target.item?.itemIndex,
      selectedVerticalStrategy: target.strategy,
      availableTargetRowItems: targetItems.map((item) => item.focusId),
      verticalFallbackReason: target.fallbackReason,
    });
  }

  private updateHierarchyDebug(
    current: FocusEntry,
    targetFocusId?: string,
    direction?: NavigationDirection,
    transitionReason?: string,
    restoreReason?: string,
  ): void {
    const region = current.navigationRegion;
    if (!region) return;
    const snapshot = this.hierarchyNavigation.getSnapshot();
    useNavigationStore.getState().updateDebug({
      activeNavigationLevel: region.regionId,
      parentRegionId: region.parentRegionId,
      childRegionId: region.childRegionId,
      lastFocusedByRegion: snapshot.lastFocusedByRegion,
      entryFocusId: region.entryFocusId,
      exitFocusId: region.exitFocusId,
      selectedMainTab:
        region.regionId === "main-navigation" ? current.focusId : undefined,
      hierarchyTransitionReason:
        transitionReason ??
        (direction === "up"
          ? "content-navigation"
          : direction === "down"
            ? "main-navigation"
            : undefined),
      hierarchyRestoreReason: restoreReason,
      resolvedCandidate: targetFocusId,
    });
  }

  private setFocus(
    entry: FocusEntry | null,
    options?: FocusChangeOptions,
  ): void {
    const state = useNavigationStore.getState();
    if (state.activeFocusId === entry?.focusId) {
      if (entry) this.focusDomElement(entry, true);
      return;
    }
    if (entry?.rowNavigation) {
      const rect = this.registry.getRect(entry);
      this.rowNavigation.recordFocus(
        {
          ...entry.rowNavigation,
          disabled: Boolean(entry.disabled),
          hidden: Boolean(entry.hidden),
          centerX: rect.left + rect.width / 2,
          focusId: entry.focusId,
        },
        options,
      );
      const rowState = this.rowNavigation.getState(entry.rowNavigation.groupId);
      useNavigationStore.getState().updateDebug({
        activeHomeRowId: entry.rowNavigation.rowId,
        activeHomeRowIndex: rowState?.activeRowIndex,
        activeHomeItemIndex: rowState?.activeItemIndex,
        preferredHomeItemIndex: rowState?.preferredItemIndex,
        preferredHomeCenterX: rowState?.preferredCenterX,
        restoredHomeRowIndex: options?.restored
          ? entry.rowNavigation.rowIndex
          : undefined,
        restoredHomeItemIndex: options?.restored
          ? entry.rowNavigation.itemIndex
          : undefined,
      });
    }
    if (entry?.gridNavigation?.index !== undefined) {
      this.logicalGridIndices.set(
        entry.gridNavigation.groupId,
        entry.gridNavigation.index,
      );
      this.updateGridDebug(entry.gridNavigation, entry.gridNavigation.index);
    }
    if (entry?.navigationRegion) {
      const preferredItemIndex = entry.rowNavigation
        ? this.rowNavigation.getState(entry.rowNavigation.groupId)
            ?.preferredItemIndex
        : entry.gridNavigation?.index;
      this.hierarchyNavigation.recordFocus(
        entry.navigationRegion,
        entry.focusId,
        preferredItemIndex,
      );
      this.updateHierarchyDebug(
        entry,
        undefined,
        undefined,
        options?.restored ? "scope-restored" : undefined,
        options?.restored ? "restored-focus" : undefined,
      );
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
    if (entry) {
      markPerformance("logical-focus-updated");
      measurePerformance(
        "input-to-focus",
        "input-received",
        "logical-focus-updated",
      );
    }
    this.syncTabStops();
    if (entry) {
      entry.element.dataset.active = "true";
      entry.element.tabIndex = 0;
    }
    entry?.onFocus?.();
    if (entry) {
      this.focusDomElement(entry, true);
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
      markPerformance("scroll-completed");
    }
    const scope = useNavigationStore.getState().activeScopeId
      ? this.scopes.get(useNavigationStore.getState().activeScopeId ?? "")
      : undefined;
    if (scope) this.updateScopeDebug(scope);
  }

  private focusDomElement(entry: FocusEntry, allowRetry: boolean): void {
    try {
      entry.element.focus({ preventScroll: true });
    } catch {
      this.recordFocusFailure(entry, "focus-threw", allowRetry);
      return;
    }
    const inputMode = useNavigationStore.getState().inputMode;
    const domFocusId =
      document.activeElement?.getAttribute("data-focus-id") ?? undefined;
    useNavigationStore.getState().updateDebug({
      domActiveElementFocusId: domFocusId,
    });
    if (inputMode === "mouse" || document.activeElement === entry.element) {
      markPerformance("dom-focus-confirmed");
      measurePerformance(
        "focus-to-dom",
        "logical-focus-updated",
        "dom-focus-confirmed",
      );
      useNavigationStore.getState().updateDebug({
        lastFocusFailureReason: undefined,
      });
      return;
    }
    this.recordFocusFailure(entry, "dom-focus-mismatch", allowRetry);
  }

  private recordFocusFailure(
    entry: FocusEntry,
    reason: string,
    allowRetry: boolean,
  ): void {
    useNavigationStore.getState().updateDebug({
      lastFocusFailureReason: reason,
      domActiveElementFocusId:
        document.activeElement?.getAttribute("data-focus-id") ?? undefined,
    });
    if (import.meta.env.DEV) {
      console.warn(`[navigation] failed to focus ${entry.focusId}: ${reason}`);
    }
    if (!allowRetry || this.focusRetryFrame !== null) return;
    const focusId = entry.focusId;
    this.focusRetryFrame = window.requestAnimationFrame(() => {
      this.focusRetryFrame = null;
      const current = this.registry.get(focusId);
      const state = useNavigationStore.getState();
      if (
        !current ||
        state.activeFocusId !== focusId ||
        state.activeScopeId !== current.scopeId ||
        !this.isValidEntry(current, current.scopeId)
      ) {
        return;
      }
      this.focusDomElement(current, false);
    });
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
