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
    this.pendingOpeners.set(scopeId, openerFocusId);
    if (this.scopes.has(scopeId)) this.activateScope(scopeId, openerFocusId);
  }

  public activateScope(scopeId: string, preferredFocusId?: string): boolean {
    const scope = this.scopes.get(scopeId);
    if (!scope) return false;
    useNavigationStore.getState().setActiveScopeId(scopeId);

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
    if (
      activateScope &&
      useNavigationStore.getState().activeScopeId !== entry.scopeId
    ) {
      this.activateScope(entry.scopeId, focusId);
      return true;
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
      const fallback =
        direction === "up" || direction === "left"
          ? entries[entries.length - 1]
          : entries[0];
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
