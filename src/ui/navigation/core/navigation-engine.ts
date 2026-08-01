import { ACTION_TO_DIRECTION, isDirectionAction } from "./navigation-actions";
import { canRestoreFocus } from "./focus-restoration";
import { FocusHistory } from "./focus-history";
import { FocusRegistry } from "./focus-registry";
import { findSpatialCandidate } from "./spatial-navigation";
import { useNavigationStore } from "../../../stores/navigation-store";
import type {
  FocusEntry,
  NavigationAction,
  ScopeRegistration,
} from "./navigation-types";
import type { FocusScrollManager } from "../scroll/focus-scroll-manager";

interface RegisteredScope extends Omit<ScopeRegistration, "parentScopeId"> {
  parentScopeId: string | null;
  openerFocusId?: string;
}

export class NavigationEngine {
  private readonly scopes = new Map<string, RegisteredScope>();
  private readonly pendingOpeners = new Map<string, string | undefined>();
  private readonly focusHistory = new FocusHistory();

  public constructor(
    public readonly registry: FocusRegistry,
    private readonly scrollManager: FocusScrollManager,
  ) {}

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
    const current = state.activeFocusId
      ? this.registry.get(state.activeFocusId)
      : undefined;
    if (!scopeId) return false;
    if (!current || current.scopeId !== scopeId) {
      return this.activateScope(scopeId);
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

  private back(): boolean {
    const scopeId = useNavigationStore.getState().activeScopeId;
    const scope = scopeId ? this.scopes.get(scopeId) : undefined;
    if (scope?.onBack?.() === true) return true;
    return false;
  }

  private setFocus(entry: FocusEntry | null): void {
    const state = useNavigationStore.getState();
    if (state.activeFocusId === entry?.focusId) return;
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
