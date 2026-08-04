import { ACTION_TO_DIRECTION, isDirectionAction } from "./navigation-actions";
import { FocusRegistry } from "./focus-registry";
import { findSpatialCandidate } from "./spatial-navigation";
import { useNavigationStore } from "../../../stores/navigation-store";
import type {
  FocusEntry,
  FocusReason,
  InputSource,
  NavigationContext,
  NavigationAction,
  NavigationDirection,
  PrimaryNavigationBlockReason,
  ScopeRegistration,
  ScopeLifecycleState,
} from "./navigation-types";
import {
  NavigationTrace,
  type NavigationTraceRecord,
} from "./navigation-trace";
import type { FocusScrollManager } from "../scroll/focus-scroll-manager";
import { getColumn, getDirectionalTarget, getRow } from "./virtual-grid";
import {
  markPerformance,
  measurePerformance,
} from "../../performance/performance-marks";
import {
  NavigationRowCoordinator,
  type HomeNavigationState,
  type HomeRowItem,
  type RowNavigationRegistration,
} from "./row-navigation";
import {
  NavigationLevelCoordinator,
  type NavigationRegionEntry,
} from "./navigation-hierarchy";
import { navigationRuntimeTrace } from "../debug/navigation-runtime-trace";

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
  transactionId: string;
  inputSource: InputSource;
}

interface PendingScopeActivation {
  requestId: number;
  scopeId: string;
  preferredFocusId?: string;
  transactionId?: string;
  inputSource: InputSource;
  focusReason: FocusReason;
  context?: NavigationContext;
  generationId: number;
  restoreOwner: string;
  transitionId?: string;
  fallbackRequested?: boolean;
  fallbackFocusId?: string;
}

interface RestoreTransaction {
  transactionId: string;
  transitionId: string;
  sourceScopeId: string;
  targetScopeId: string;
  preferredFocusId: string;
  context: NavigationContext;
  generationId: number;
  restoreOwner: string;
  status: "requested" | "waiting" | "committed" | "cancelled";
}

interface PendingRestoreInput {
  direction: NavigationDirection;
  inputSource: InputSource;
}

interface FocusChangeOptions {
  preservePreferredPosition?: boolean;
  restored?: boolean;
  transactionId?: string;
  inputSource?: InputSource;
  focusReason?: FocusReason;
  context?: NavigationContext;
  generationId?: number;
}

interface PendingRegionFocusRequest {
  requestId: number;
  scopeId: string;
  regionId: string;
  targetFocusId: string;
  direction: NavigationDirection;
  transactionId: string;
  inputSource: InputSource;
}

type TraceFields = Omit<
  NavigationTraceRecord,
  "event" | "transactionId" | "timestamp" | "inputSource" | "direction"
>;

interface ActiveNavigationTrace {
  transactionId: string;
  inputSource: InputSource;
  direction: NavigationDirection;
  base: TraceFields;
}

export class NavigationEngine {
  private readonly scopes = new Map<string, RegisteredScope>();
  private readonly pendingOpeners = new Map<string, string | undefined>();
  private readonly restoreTransactions = new Map<string, RestoreTransaction>();
  private readonly contexts = new Map<string, NavigationContext>();
  private readonly scopeOpenContexts = new Map<string, NavigationContext>();
  private readonly logicalGridIndices = new Map<string, number>();
  private readonly rowNavigation = new NavigationRowCoordinator();
  private readonly hierarchyNavigation = new NavigationLevelCoordinator();
  private readonly trace: NavigationTrace;
  private readonly traceBases = new Map<string, TraceFields>();
  private readonly unregisterRegistryListener: () => void;
  private readonly scopeWatchdogFrames = new Map<string, number>();
  private readonly scopeWatchdogWarnings = new Set<string>();
  private pendingGridFocus: PendingGridFocusRequest | null = null;
  private pendingRegionFocus: PendingRegionFocusRequest | null = null;
  private pendingScopeActivation: PendingScopeActivation | null = null;
  private pendingRestoreInput: PendingRestoreInput | null = null;
  private pendingFrame: number | null = null;
  private pendingTimeout: number | null = null;
  private focusRetryFrame: number | null = null;
  private nextGridRequestId = 0;
  private nextScopeRequestId = 0;
  private nextRegionRequestId = 0;
  private nextGenerationId = 0;
  private activeNavigationTrace: ActiveNavigationTrace | null = null;

  public constructor(
    public readonly registry: FocusRegistry,
    private readonly scrollManager: FocusScrollManager,
    trace?: NavigationTrace,
  ) {
    this.trace = trace ?? new NavigationTrace();
    this.unregisterRegistryListener = registry.subscribe(() => {
      this.syncActiveGridIndex();
      this.tryCompletePendingGridFocus();
      this.tryCompletePendingRegionFocus();
      this.tryCompletePendingScopeActivation();
      this.runScopeWatchdog();
    });
  }

  public getNavigationTrace(): NavigationTraceRecord[] {
    return this.trace.getRecords();
  }

  public clearNavigationTrace(): void {
    this.trace.clear();
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
    this.pendingRestoreInput = null;
    this.restoreTransactions.clear();
    this.contexts.clear();
    this.scopeOpenContexts.clear();
    this.rowNavigation.reset();
    this.hierarchyNavigation.reset();
    this.traceBases.clear();
    this.activeNavigationTrace = null;
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
    navigationRuntimeTrace.record("scope_register", {
      details: {
        scopeId: scope.scopeId,
        parentScopeId: parentScopeId ?? null,
        initialFocusId: scope.initialFocusId ?? null,
        lifecycle: registered.lifecycleState,
      },
    });
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
    if (
      this.pendingScopeActivation?.scopeId === scopeId &&
      !this.pendingScopeActivation.transitionId
    ) {
      this.cancelPendingScopeActivation(scopeId);
    }
    scope.lifecycleState = "unmounting";
    const wasActive = useNavigationStore.getState().activeScopeId === scopeId;
    if (scope.openerFocusId) {
      this.pendingOpeners.set(scopeId, scope.openerFocusId);
    }
    this.scopes.delete(scopeId);
    navigationRuntimeTrace.record("scope_unregister", {
      details: {
        scopeId,
        parentScopeId: scope.parentScopeId ?? null,
        wasActive,
      },
    });

    if (wasActive) {
      useNavigationStore.getState().setActiveScopeId(null);
      this.setFocus(null);
      const restored = this.tryCompletePendingScopeActivation();
      if (!restored && !this.pendingScopeActivation && scope.parentScopeId) {
        const parentScope = this.scopes.get(scope.parentScopeId);
        if (parentScope) {
          const openerFocusId =
            scope.restoreFocus &&
            scope.openerFocusId &&
            this.isValidFocusId(scope.openerFocusId, scope.parentScopeId)
              ? scope.openerFocusId
              : undefined;
          this.requestScopeActivation(
            scope.parentScopeId,
            openerFocusId,
            openerFocusId ? "route-restore" : "initial-focus",
            "programmatic",
            "scope-unmount-fallback",
          );
        }
      }
    }
  }

  public requestScopeRestore(
    sourceScopeId: string,
    targetScopeId: string,
    transitionId: string,
  ): boolean {
    const sourceScope = this.scopes.get(sourceScopeId);
    const context =
      this.scopeOpenContexts.get(sourceScopeId) ??
      this.contexts.get(targetScopeId);
    const preferredFocusId = context?.focusId ?? sourceScope?.openerFocusId;
    if (!sourceScope || !context || !preferredFocusId) return false;

    const existing = this.restoreTransactions.get(transitionId);
    if (existing && existing.status !== "cancelled") {
      const pending = this.pendingScopeActivation;
      if (pending?.transitionId === transitionId) {
        existing.status = "waiting";
        const activeFocusId = useNavigationStore.getState().activeFocusId;
        this.trace.emit(
          "CONTEXT_RESTORE_REQUEST_REUSED",
          existing.transactionId,
          {
            ...this.getTraceFields(
              activeFocusId ? this.registry.get(activeFocusId) : undefined,
              targetScopeId,
            ),
            inputSource: "programmatic",
            direction: null,
            targetRegionId: context.regionId ?? null,
            selectedFocusId: preferredFocusId,
            selectedItemIndex: context.itemIndex ?? null,
            preferredItemIndexAfter: context.preferredItemIndex ?? null,
            focusReason: "route-restore",
            generationId: existing.generationId,
            pendingRestore: true,
            restoreOwner: existing.restoreOwner,
            restoreCommitCount: 0,
            restoreRequestReuseReason: "same-transition",
            contextBefore: context,
            contextAfter: null,
          },
        );
        return this.tryCompletePendingScopeActivation();
      }
      const activeFocusId = useNavigationStore.getState().activeFocusId;
      this.trace.emit(
        "CONTEXT_RESTORE_REQUEST_REUSED",
        existing.transactionId,
        {
          ...this.getTraceFields(
            activeFocusId ? this.registry.get(activeFocusId) : undefined,
            targetScopeId,
          ),
          inputSource: "programmatic",
          direction: null,
          targetRegionId: context.regionId ?? null,
          selectedFocusId: preferredFocusId,
          selectedItemIndex: context.itemIndex ?? null,
          preferredItemIndexAfter: context.preferredItemIndex ?? null,
          focusReason: "route-restore",
          generationId: existing.generationId,
          pendingRestore: false,
          restoreOwner: existing.restoreOwner,
          restoreCommitCount: existing.status === "committed" ? 1 : 0,
          restoreRequestReuseReason: "same-transition",
          contextBefore: context,
          contextAfter: this.contexts.get(targetScopeId) ?? null,
        },
      );
      return existing.status === "committed";
    }

    if (this.pendingScopeActivation?.transitionId) {
      this.cancelRestoreTransaction(
        this.pendingScopeActivation.transitionId,
        "new-transition",
      );
    }

    const restoreOwner = "route-transition";
    const activeFocusId = useNavigationStore.getState().activeFocusId;
    const restoreBase = this.getTraceFields(
      activeFocusId ? this.registry.get(activeFocusId) : undefined,
      targetScopeId,
    );
    const transactionId = this.beginTrace("programmatic", null, restoreBase);
    const transaction: RestoreTransaction = {
      transactionId,
      transitionId,
      sourceScopeId,
      targetScopeId,
      preferredFocusId,
      context,
      generationId: this.allocateGeneration(),
      restoreOwner,
      status: "requested",
    };
    this.restoreTransactions.set(transitionId, transaction);
    this.trace.emit("CONTEXT_RESTORE_BEGIN", transactionId, {
      ...restoreBase,
      inputSource: "programmatic",
      direction: null,
      targetRegionId: context.regionId ?? null,
      selectedFocusId: preferredFocusId,
      selectedItemIndex: context.itemIndex ?? null,
      preferredItemIndexAfter: context.preferredItemIndex ?? null,
      focusReason: "route-restore",
      generationId: transaction.generationId,
      pendingRestore: true,
      restoreOwner,
      restoreCommitCount: 0,
      contextBefore: context,
      contextAfter: null,
    });
    return this.requestScopeActivation(
      targetScopeId,
      preferredFocusId,
      "route-restore",
      "programmatic",
      restoreOwner,
      transaction,
    );
  }

  public notifyRouteActive(scopeId: string): boolean {
    const pending = this.pendingScopeActivation;
    if (!pending || pending.scopeId !== scopeId) return false;
    return this.tryCompletePendingScopeActivation();
  }

  public prepareScopeOpen(scopeId: string, openerFocusId?: string): void {
    const activeScopeId = useNavigationStore.getState().activeScopeId;
    const opener = openerFocusId ? this.registry.get(openerFocusId) : undefined;
    if (
      activeScopeId &&
      activeScopeId !== scopeId &&
      this.scopes.get(activeScopeId)?.modal &&
      opener?.scopeId !== activeScopeId
    ) {
      return;
    }
    this.cancelPendingVirtualFocus("scope-open");
    if (openerFocusId) {
      const state = useNavigationStore.getState();
      const opener = this.registry.get(openerFocusId);
      const current = state.activeFocusId
        ? this.registry.get(state.activeFocusId)
        : undefined;
      const saveBase = this.getTraceFields(current, state.activeScopeId);
      const context = this.captureContext(current, state.activeScopeId);
      if (context) {
        this.contexts.set(context.scopeId, context);
        this.scopeOpenContexts.set(scopeId, context);
      }
      const saveTransactionId = this.beginTrace(
        state.inputMode,
        null,
        saveBase,
      );
      this.trace.emit("CONTEXT_SAVE", saveTransactionId, {
        ...saveBase,
        inputSource: state.inputMode,
        direction: null,
        scopeId: state.activeScopeId,
        targetRegionId: opener?.navigationRegion?.regionId ?? null,
        selectedFocusId: openerFocusId,
        selectedItemIndex: this.getItemIndex(opener),
        preferredItemIndexAfter: this.getPreferredItemIndex(opener),
        focusReason: "route-restore",
        generationId: context?.generationId ?? null,
        restoreOwner: "prepare-scope-open",
        pendingRestore: false,
        restoreCommitCount: 0,
        memoryGenerationId: null,
        memoryDecision: null,
        memoryRejectionReason: null,
        contextBefore: context,
        contextAfter: context,
      });
    }
    this.pendingOpeners.set(scopeId, openerFocusId);
    if (this.scopes.has(scopeId)) this.requestScopeActivation(scopeId);
  }

  public activateScope(scopeId: string, preferredFocusId?: string): boolean {
    const activeScopeId = useNavigationStore.getState().activeScopeId;
    if (
      activeScopeId &&
      activeScopeId !== scopeId &&
      this.scopes.get(activeScopeId)?.modal &&
      !this.isAdjacentScope(activeScopeId, scopeId)
    ) {
      return false;
    }
    const scope = this.scopes.get(scopeId);
    if (!scope) return false;
    return this.requestScopeActivation(
      scopeId,
      preferredFocusId,
      preferredFocusId ? "route-restore" : "initial-focus",
      "programmatic",
      "component-request",
    );
  }

  private isAdjacentScope(
    firstScopeId: string,
    secondScopeId: string,
  ): boolean {
    const second = this.scopes.get(secondScopeId);
    return second?.parentScopeId === firstScopeId;
  }

  public completePendingRestore(
    scopeId: string,
    fallbackFocusId?: string,
  ): boolean {
    const pending = this.pendingScopeActivation;
    if (!pending || pending.scopeId !== scopeId || !pending.preferredFocusId) {
      return false;
    }
    const scope = this.scopes.get(scopeId);
    if (!scope) return false;
    const fallbackId =
      fallbackFocusId ??
      (scope.initialFocusId &&
      this.isValidFocusId(scope.initialFocusId, scopeId)
        ? scope.initialFocusId
        : this.registry.getScopeEntries(scopeId)[0]?.focusId);
    if (!fallbackId) return false;
    pending.preferredFocusId = undefined;
    pending.context = undefined;
    pending.fallbackRequested = true;
    pending.fallbackFocusId = fallbackId;
    pending.focusReason = "region-fallback";
    return this.tryCompletePendingScopeActivation();
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
    focusReason: FocusReason = "initial-focus",
    inputSource: InputSource = "programmatic",
    restoreOwner = "engine",
    restoreTransaction?: RestoreTransaction,
  ): boolean {
    const scope = this.scopes.get(scopeId);
    if (!scope) return false;
    const existing = this.pendingScopeActivation;
    if (existing?.transitionId && !restoreTransaction) {
      return false;
    }
    if (
      existing &&
      existing.scopeId === scopeId &&
      existing.preferredFocusId === preferredFocusId &&
      existing.transitionId === restoreTransaction?.transitionId
    ) {
      return this.tryCompletePendingScopeActivation();
    }
    if (existing?.transitionId) {
      this.cancelRestoreTransaction(existing.transitionId, "new-transition");
    } else {
      this.cancelPendingScopeActivation();
    }
    const context =
      restoreTransaction?.context ??
      (preferredFocusId ? this.contexts.get(scopeId) : undefined);
    const generationId =
      restoreTransaction?.generationId ??
      (preferredFocusId
        ? this.allocateGeneration()
        : this.getOrCreateGeneration(scopeId));
    const request: PendingScopeActivation = {
      requestId: ++this.nextScopeRequestId,
      scopeId,
      preferredFocusId,
      transactionId: restoreTransaction?.transactionId,
      inputSource,
      focusReason,
      context,
      generationId,
      restoreOwner,
      transitionId: restoreTransaction?.transitionId,
    };
    if (preferredFocusId && !restoreTransaction) {
      const state = useNavigationStore.getState();
      const currentEntry = state.activeFocusId
        ? this.registry.get(state.activeFocusId)
        : undefined;
      const restoreBase = {
        ...this.getTraceFields(currentEntry, scopeId),
        generationId,
      };
      const transactionId = this.beginTrace(inputSource, null, restoreBase);
      request.transactionId = transactionId;
      this.trace.emit("CONTEXT_RESTORE_BEGIN", transactionId, {
        ...restoreBase,
        inputSource,
        direction: null,
        targetRegionId:
          this.registry.get(preferredFocusId)?.navigationRegion?.regionId ??
          null,
        selectedFocusId: preferredFocusId,
        selectedItemIndex: this.getItemIndex(
          this.registry.get(preferredFocusId),
        ),
        focusReason,
        generationId,
        pendingRestore: true,
        restoreOwner,
        restoreCommitCount: 0,
        memoryGenerationId: null,
        memoryDecision: null,
        memoryRejectionReason: null,
        contextBefore: this.contexts.get(scopeId) ?? null,
        contextAfter: null,
      });
    }
    this.pendingScopeActivation = request;
    if (restoreTransaction) restoreTransaction.status = "waiting";
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
      this.scopes.get(activeScopeId)?.modal &&
      !this.isAdjacentScope(activeScopeId, pending.scopeId)
    ) {
      return false;
    }

    const preferredInScope =
      pending.preferredFocusId &&
      this.isValidFocusId(pending.preferredFocusId, pending.scopeId)
        ? pending.preferredFocusId
        : undefined;
    const initialFocus =
      scope.initialFocusId &&
      this.isValidFocusId(scope.initialFocusId, pending.scopeId)
        ? scope.initialFocusId
        : undefined;
    const explicitRestorePending =
      Boolean(pending.preferredFocusId) && !pending.fallbackRequested;
    const focusId =
      preferredInScope ??
      (explicitRestorePending
        ? undefined
        : (pending.fallbackFocusId ??
          initialFocus ??
          this.registry.getScopeEntries(pending.scopeId)[0]?.focusId));
    const resolvedFocusReason: FocusReason =
      explicitRestorePending && !preferredInScope
        ? "region-fallback"
        : pending.focusReason;

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
    const restoreContext =
      pending.context ??
      this.captureContext(entry, pending.scopeId, pending.generationId);
    this.setFocus(entry, {
      restored: Boolean(pending.preferredFocusId || pending.context),
      transactionId: pending.transactionId,
      inputSource: pending.inputSource,
      focusReason: resolvedFocusReason,
      context: restoreContext,
      generationId: pending.generationId,
    });
    navigationRuntimeTrace.record("scope_active", {
      pendingRestore: false,
      restoreTarget: pending.preferredFocusId ?? null,
      restoreOwner: pending.restoreOwner,
      generationId: pending.generationId,
      inputSource: pending.inputSource,
      focusReason: pending.focusReason,
      details: {
        scopeId: pending.scopeId,
        requestId: pending.requestId,
        wasRestore: Boolean(pending.transitionId || pending.preferredFocusId),
      },
    });
    if (pending.transactionId) {
      if (pending.transitionId) {
        const restoreTransaction = this.restoreTransactions.get(
          pending.transitionId,
        );
        if (restoreTransaction) {
          restoreTransaction.status = "committed";
          this.scopeOpenContexts.delete(restoreTransaction.sourceScopeId);
        }
      }
      const restoreFields = this.getTraceFields(entry, pending.scopeId);
      this.trace.emit("CONTEXT_RESTORE_COMMIT", pending.transactionId, {
        ...restoreFields,
        inputSource: pending.inputSource,
        direction: null,
        targetRegionId: entry.navigationRegion?.regionId ?? null,
        selectedFocusId: entry.focusId,
        selectedItemIndex: this.getItemIndex(entry),
        preferredItemIndexAfter: this.getPreferredItemIndex(entry),
        focusReason: resolvedFocusReason,
        generationId: pending.generationId,
        pendingRestore: false,
        restoreOwner: pending.restoreOwner,
        restoreCommitCount: 1,
        contextBefore: pending.context ?? null,
        contextAfter: this.contexts.get(pending.scopeId) ?? null,
      });
    }
    this.updateScopeDebug(scope);
    this.replayPendingRestoreInput();
    return useNavigationStore.getState().activeFocusId === focusId;
  }

  private cancelPendingScopeActivation(scopeId?: string): void {
    const pending = this.pendingScopeActivation;
    if (!pending || (scopeId && pending.scopeId !== scopeId)) return;
    const scope = this.scopes.get(pending.scopeId);
    if (scope && scope.lifecycleState !== "unmounting") {
      scope.lifecycleState = "suspended";
    }
    navigationRuntimeTrace.record("restore_cancel", {
      pendingRestore: Boolean(pending.transitionId),
      restoreTarget: pending.preferredFocusId ?? null,
      restoreOwner: pending.restoreOwner,
      details: { scopeId: pending.scopeId, reason: "pending-scope-cancel" },
    });
    this.pendingScopeActivation = null;
    useNavigationStore.getState().updateDebug({
      pendingScopeActivationRequestId: undefined,
    });
  }

  private cancelRestoreTransaction(
    transitionId: string,
    reason: "new-transition",
  ): void {
    const transaction = this.restoreTransactions.get(transitionId);
    if (transaction) transaction.status = "cancelled";
    navigationRuntimeTrace.record("restore_cancel", {
      pendingRestore: false,
      restoreTarget: transaction?.preferredFocusId ?? null,
      restoreOwner: transaction?.restoreOwner ?? null,
      details: { transitionId, reason },
    });
    if (this.pendingScopeActivation?.transitionId === transitionId) {
      this.pendingScopeActivation = null;
      useNavigationStore.getState().updateDebug({
        pendingScopeActivationRequestId: undefined,
        fallbackReason: `restore-cancelled-${reason}`,
      });
    }
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

  public getPrimaryNavigationBlockReason(): PrimaryNavigationBlockReason | null {
    const state = useNavigationStore.getState();
    const activeScope = state.activeScopeId
      ? this.scopes.get(state.activeScopeId)
      : undefined;
    if (activeScope?.modal || activeScope?.trapFocus) return "modal";
    if (
      this.pendingScopeActivation &&
      state.activeScopeId !== this.pendingScopeActivation.scopeId
    ) {
      return this.pendingScopeActivation.transitionId
        ? "restoration-pending"
        : "transition-pending";
    }
    if (this.pendingGridFocus || this.pendingRegionFocus) {
      return "transition-pending";
    }
    return null;
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

  public focusFromPointer(focusId: string): boolean {
    const state = useNavigationStore.getState();
    const currentEntry = state.activeFocusId
      ? this.registry.get(state.activeFocusId)
      : undefined;
    const targetEntry = this.registry.get(focusId);
    const base = this.getTraceFields(currentEntry, targetEntry?.scopeId);
    const transactionId = this.beginTrace("mouse", null, base);
    this.trace.emit("POINTER_SELECTION", transactionId, {
      ...base,
      inputSource: "mouse",
      direction: null,
      targetRegionId: targetEntry?.navigationRegion?.regionId ?? null,
      selectedFocusId: focusId,
      selectedItemIndex: this.getItemIndex(targetEntry),
      focusReason: "pointer-selection",
    });
    return this.focus(focusId, true, {
      transactionId,
      inputSource: "mouse",
      focusReason: "pointer-selection",
    });
  }

  public dispatch(
    action: NavigationAction,
    inputSource: InputSource = "programmatic",
  ): boolean {
    useNavigationStore.getState().recordAction(action);
    const activeScopeId = useNavigationStore.getState().activeScopeId;
    const activeScope = activeScopeId
      ? this.scopes.get(activeScopeId)
      : undefined;
    if (activeScope?.onAction?.(action, inputSource) === true) return true;
    if (this.isInputBlockedForPendingScope()) {
      if (isDirectionAction(action)) {
        this.pendingRestoreInput = {
          direction: ACTION_TO_DIRECTION[action] ?? "down",
          inputSource,
        };
        const state = useNavigationStore.getState();
        const activeEntry = state.activeFocusId
          ? this.registry.get(state.activeFocusId)
          : undefined;
        const base = this.getTraceFields(
          activeEntry,
          state.activeScopeId ?? this.pendingScopeActivation?.scopeId,
        );
        const transactionId = this.beginTrace(
          inputSource,
          this.pendingRestoreInput.direction,
          base,
        );
        this.trace.emit("NAV_INPUT_BLOCKED", transactionId, {
          ...base,
          inputSource,
          direction: this.pendingRestoreInput.direction,
          resolutionStrategy: "restore-pending",
          focusReason: "route-restore",
          pendingRestore: true,
          restoreOwner: this.pendingScopeActivation?.restoreOwner ?? null,
          restoreCommitCount: 0,
        });
        return true;
      }
      return false;
    }
    if (isDirectionAction(action)) {
      return this.move(ACTION_TO_DIRECTION[action] ?? "down", inputSource);
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

  private replayPendingRestoreInput(): void {
    const pendingInput = this.pendingRestoreInput;
    if (!pendingInput) return;
    this.pendingRestoreInput = null;
    this.move(pendingInput.direction, pendingInput.inputSource);
  }

  private move(
    direction: NavigationDirection,
    inputSource: InputSource,
  ): boolean {
    const state = useNavigationStore.getState();
    const currentEntry = state.activeFocusId
      ? this.registry.get(state.activeFocusId)
      : undefined;
    const base = this.getTraceFields(currentEntry, state.activeScopeId);
    base.focusReason = "directional-navigation";
    const transactionId = this.beginTrace(inputSource, direction, base);
    this.activeNavigationTrace = {
      transactionId,
      inputSource,
      direction,
      base,
    };
    try {
      return this.resolveMove(direction);
    } finally {
      this.activeNavigationTrace = null;
    }
  }

  private resolveMove(direction: NavigationDirection): boolean {
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

    const override = current.navigation?.[direction];
    if (override && this.focus(override)) {
      this.recordResolution(direction, override, [], 0, "override");
      return true;
    }

    if (
      current.navigationRegion?.childRegionId &&
      direction === "down" &&
      !current.rowNavigation &&
      !current.gridNavigation
    ) {
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
            "linear",
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
        "linear",
      );
      return target ? this.focus(target.focusId) : false;
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
      "spatial",
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
      this.getContextState(current, currentRow),
      this.contexts.get(scopeId)?.generationId,
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
      target.memoryDecision === "rejected"
        ? "row-memory-rejected"
        : `row-vertical/${target.strategy}`,
      target,
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
    return (
      this.getContextForRegion(regionId)?.focusId ??
      this.hierarchyNavigation.getLastFocusedFocusId(regionId)
    );
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
      ? (this.getContextForRegion(region.childRegionId)?.focusId ??
        this.hierarchyNavigation.getLastFocusedFocusId(region.childRegionId))
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
      return this.focus(targetFocusId, true, {
        focusReason: "region-fallback",
        inputSource: this.activeNavigationTrace?.inputSource,
        transactionId: this.activeNavigationTrace?.transactionId,
      });
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
    const useGamepadParent =
      this.activeNavigationTrace?.inputSource === "gamepad" &&
      Boolean(region.gamepadParentRegionId);
    const parentRegion = useGamepadParent
      ? {
          ...region,
          parentRegionId: region.gamepadParentRegionId,
          exitFocusId: region.gamepadExitFocusId,
        }
      : region;
    const entries = this.getRegionEntries(scopeId);
    const targetFocusId = this.hierarchyNavigation.resolveParent(
      parentRegion,
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
    return this.focus(targetFocusId, true, {
      focusReason: "region-fallback",
      inputSource: this.activeNavigationTrace?.inputSource,
      transactionId: this.activeNavigationTrace?.transactionId,
    });
  }

  private isAtGridTop(entry: FocusEntry): boolean {
    const grid = entry.gridNavigation;
    if (!grid) return false;
    const index =
      grid.index ?? this.logicalGridIndices.get(grid.groupId) ?? undefined;
    return index !== undefined && getRow(index, grid.columns) === 0;
  }

  private syncActiveGridIndex(): void {
    const activeFocusId = useNavigationStore.getState().activeFocusId;
    const activeEntry = activeFocusId
      ? this.registry.get(activeFocusId)
      : undefined;
    const grid = activeEntry?.gridNavigation;
    if (!grid || grid.index === undefined) return;
    this.logicalGridIndices.set(grid.groupId, grid.index);
    this.updateGridDebug(grid, grid.index);
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
      this.getContextForRegion(regionId)?.preferredItemIndex ??
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
      transactionId: this.activeNavigationTrace?.transactionId ?? "nav-unknown",
      inputSource: this.activeNavigationTrace?.inputSource ?? "programmatic",
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
    this.focus(entry.focusId, true, {
      transactionId: pending.transactionId,
      inputSource: pending.inputSource,
      focusReason: "directional-navigation",
    });
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
      this.recordResolution(direction, undefined, [], 0, "grid");
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
        this.recordResolution(
          direction,
          focusId,
          [],
          0,
          "grid-materialization",
        );
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
        this.recordResolution(direction, candidate.focusId, [], 0, "grid");
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

    this.recordResolution(direction, undefined, [], 0, "grid");
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
    navigationRuntimeTrace.record("scope_watchdog", {
      details: {
        scopeId: scope.scopeId,
        registeredFocusables: entries.length,
        activeFocusValid,
      },
    });
    navigationRuntimeTrace.record("requestAnimationFrame", {
      details: { reason: "scope-watchdog" },
    });
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
        navigationRuntimeTrace.record("fallback", {
          restoreTarget: recoveryId,
          restoreOwner: "scope-watchdog",
          focusReason: "region-fallback",
          details: { scopeId: scope.scopeId, reason: "watchdog-recovery" },
        });
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
      navigationRuntimeTrace.record("fallback", {
        restoreOwner: "scope-watchdog",
        focusReason: "region-fallback",
        details: { scopeId: scope.scopeId, reason: "watchdog-no-focusable" },
      });
      useNavigationStore.getState().setActiveFocusId(null);
      useNavigationStore.getState().updateDebug({
        scopeLifecycleState: scope.lifecycleState,
        lastFocusFailureReason: "scope-watchdog-no-focusable",
      });
      const recoveryRequest: PendingScopeActivation = {
        requestId: ++this.nextScopeRequestId,
        scopeId: scope.scopeId,
        inputSource: "programmatic",
        focusReason: "region-fallback",
        generationId: this.allocateGeneration(),
        restoreOwner: "scope-watchdog",
      };
      this.pendingScopeActivation = recoveryRequest;
      useNavigationStore.getState().updateDebug({
        pendingScopeActivationRequestId: recoveryRequest.requestId,
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
      transactionId: this.activeNavigationTrace?.transactionId ?? "nav-unknown",
      inputSource: this.activeNavigationTrace?.inputSource ?? "programmatic",
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
    navigationRuntimeTrace.record("requestAnimationFrame", {
      details: { reason: "grid-focus-commit" },
    });
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
      this.focus(target.focusId, true, {
        transactionId: current.transactionId,
        inputSource: current.inputSource,
        focusReason: "directional-navigation",
      });
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

  private beginTrace(
    inputSource: InputSource,
    direction: NavigationDirection | null,
    base: TraceFields,
  ): string {
    const transactionId = this.trace.begin(inputSource, direction, base);
    this.traceBases.set(transactionId, base);
    return transactionId;
  }

  private allocateGeneration(): number {
    return ++this.nextGenerationId;
  }

  private getOrCreateGeneration(scopeId: string): number {
    return (
      this.contexts.get(scopeId)?.generationId ?? this.allocateGeneration()
    );
  }

  private captureContext(
    entry: FocusEntry | undefined,
    scopeId: string | null | undefined,
    generationId?: number,
  ): NavigationContext | undefined {
    if (!entry || !scopeId) return undefined;
    const existing = this.contexts.get(scopeId);
    const rowState = entry.rowNavigation
      ? this.rowNavigation.getState(entry.rowNavigation.groupId)
      : undefined;
    return {
      generationId:
        generationId ?? existing?.generationId ?? this.allocateGeneration(),
      scopeId,
      regionId: entry.navigationRegion?.regionId,
      rowId: entry.rowNavigation?.rowId,
      focusId: entry.focusId,
      itemIndex: this.getItemIndex(entry) ?? undefined,
      preferredItemIndex:
        existing?.focusId === entry.focusId
          ? existing.preferredItemIndex
          : (rowState?.preferredItemIndex ??
            this.getItemIndex(entry) ??
            undefined),
      horizontalCenter:
        existing?.focusId === entry.focusId
          ? existing.horizontalCenter
          : rowState?.preferredCenterX,
    };
  }

  private updateNavigationContext(
    entry: FocusEntry,
    scopeId: string,
    generationId: number,
  ): NavigationContext {
    const rowState = entry.rowNavigation
      ? this.rowNavigation.getState(entry.rowNavigation.groupId)
      : undefined;
    const context: NavigationContext = {
      generationId,
      scopeId,
      regionId: entry.navigationRegion?.regionId,
      rowId: entry.rowNavigation?.rowId,
      focusId: entry.focusId,
      itemIndex: this.getItemIndex(entry) ?? undefined,
      preferredItemIndex:
        rowState?.preferredItemIndex ?? this.getItemIndex(entry) ?? undefined,
      horizontalCenter: rowState?.preferredCenterX,
    };
    this.contexts.set(scopeId, context);
    return context;
  }

  private getRoute(entry?: FocusEntry): string {
    return (
      entry?.element.closest<HTMLElement>("[data-view]")?.dataset.view ??
      document.querySelector<HTMLElement>("[data-view]")?.dataset.view ??
      "unknown"
    );
  }

  private getItemIndex(entry?: FocusEntry): number | null {
    return (
      entry?.rowNavigation?.itemIndex ?? entry?.gridNavigation?.index ?? null
    );
  }

  private getPreferredItemIndex(entry?: FocusEntry): number | null {
    const context = entry?.scopeId
      ? this.contexts.get(entry.scopeId)
      : undefined;
    if (context && context.focusId === entry?.focusId) {
      return context.preferredItemIndex ?? null;
    }
    if (entry?.rowNavigation) {
      return (
        this.rowNavigation.getState(entry.rowNavigation.groupId)
          ?.preferredItemIndex ?? entry.rowNavigation.itemIndex
      );
    }
    return entry?.gridNavigation?.index ?? null;
  }

  private getContextState(
    entry: FocusEntry,
    row: NonNullable<FocusEntry["rowNavigation"]>,
  ): HomeNavigationState | undefined {
    const context = this.contexts.get(entry.scopeId);
    if (!context || context.focusId !== entry.focusId) return undefined;
    return {
      activeRowIndex: row.rowIndex,
      activeItemIndex: row.itemIndex,
      preferredItemIndex: context.preferredItemIndex ?? row.itemIndex,
      preferredCenterX: context.horizontalCenter,
    };
  }

  private getContextForRegion(
    regionId: string,
    scopeId = useNavigationStore.getState().activeScopeId ?? undefined,
  ): NavigationContext | undefined {
    if (!scopeId) return undefined;
    const context = this.contexts.get(scopeId);
    return context?.regionId === regionId ? context : undefined;
  }

  private getTraceFields(
    entry: FocusEntry | undefined,
    scopeId: string | null | undefined,
  ): TraceFields {
    const rowState = entry?.rowNavigation
      ? this.rowNavigation.getState(entry.rowNavigation.groupId)
      : undefined;
    return {
      route: this.getRoute(entry),
      scopeId: scopeId ?? entry?.scopeId ?? null,
      generationId: entry?.scopeId
        ? this.getOrCreateGeneration(entry.scopeId)
        : null,
      fromRegionId: entry?.navigationRegion?.regionId ?? null,
      fromFocusId: entry?.focusId ?? null,
      fromItemIndex: this.getItemIndex(entry),
      preferredItemIndexBefore:
        rowState?.preferredItemIndex ?? this.getItemIndex(entry),
      targetRegionId: null,
      resolutionStrategy: null,
      candidatesConsidered: [],
      selectedFocusId: null,
      selectedItemIndex: null,
      preferredItemIndexAfter: null,
      focusReason: "initial-focus",
      memoryGenerationId: null,
      memoryDecision: null,
      memoryRejectionReason: null,
      pendingRestore: false,
      restoreOwner: null,
      restoreCommitCount: 0,
      restoreRequestReuseReason: null,
      contextBefore: entry?.scopeId
        ? (this.contexts.get(entry.scopeId) ?? null)
        : null,
      contextAfter: null,
    };
  }

  private setFocus(
    entry: FocusEntry | null,
    options?: FocusChangeOptions,
  ): void {
    const state = useNavigationStore.getState();
    const previousEntry = state.activeFocusId
      ? this.registry.get(state.activeFocusId)
      : undefined;
    const transactionId =
      options?.transactionId ?? this.activeNavigationTrace?.transactionId;
    const traceBase = transactionId
      ? (this.traceBases.get(transactionId) ??
        this.activeNavigationTrace?.base ??
        this.getTraceFields(previousEntry, state.activeScopeId))
      : this.getTraceFields(
          previousEntry,
          entry?.scopeId ?? state.activeScopeId,
        );
    const inputSource =
      options?.inputSource ??
      this.activeNavigationTrace?.inputSource ??
      "programmatic";
    const focusReason =
      options?.focusReason ??
      (this.activeNavigationTrace
        ? "directional-navigation"
        : options?.restored
          ? "route-restore"
          : "initial-focus");
    const generationId =
      options?.generationId ??
      (entry?.scopeId
        ? this.getOrCreateGeneration(entry.scopeId)
        : (traceBase.generationId ?? this.allocateGeneration()));
    if (state.activeFocusId === entry?.focusId) {
      if (entry) this.focusDomElement(entry, true);
      this.emitFocusCommit(
        transactionId ?? this.beginTrace(inputSource, null, traceBase),
        traceBase,
        entry,
        inputSource,
        focusReason,
        generationId,
      );
      return;
    }
    if (entry?.rowNavigation) {
      const rect = this.registry.getRect(entry);
      const rowItem = {
        ...entry.rowNavigation,
        disabled: Boolean(entry.disabled),
        hidden: Boolean(entry.hidden),
        centerX: rect.left + rect.width / 2,
        focusId: entry.focusId,
      } satisfies RowNavigationRegistration;
      if (options?.context) {
        this.rowNavigation.restoreContext(rowItem, {
          generationId,
          preferredItemIndex: options.context.preferredItemIndex,
          horizontalCenter: options.context.horizontalCenter,
        });
      } else {
        this.rowNavigation.recordFocus(rowItem, {
          ...options,
          generationId,
        });
      }
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
    const context = entry
      ? this.updateNavigationContext(entry, entry.scopeId, generationId)
      : undefined;
    if (entry?.navigationRegion) {
      const preferredItemIndex =
        context?.preferredItemIndex ?? entry.gridNavigation?.index;
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
    this.emitFocusCommit(
      transactionId ?? this.beginTrace(inputSource, null, traceBase),
      traceBase,
      entry,
      inputSource,
      focusReason,
      generationId,
    );
  }

  private emitFocusCommit(
    transactionId: string,
    traceBase: TraceFields,
    entry: FocusEntry | null,
    inputSource: InputSource,
    focusReason: FocusReason,
    generationId?: number,
  ): void {
    this.trace.emit("FOCUS_COMMIT", transactionId, {
      ...traceBase,
      inputSource,
      direction: this.activeNavigationTrace?.direction ?? null,
      route: this.getRoute(entry ?? undefined),
      scopeId: entry?.scopeId ?? traceBase.scopeId,
      targetRegionId: entry?.navigationRegion?.regionId ?? null,
      selectedFocusId: entry?.focusId ?? null,
      selectedItemIndex: this.getItemIndex(entry ?? undefined),
      preferredItemIndexAfter: this.getPreferredItemIndex(entry ?? undefined),
      focusReason,
      generationId: generationId ?? traceBase.generationId,
      contextAfter: entry?.scopeId
        ? (this.contexts.get(entry.scopeId) ?? null)
        : null,
    });
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
    navigationRuntimeTrace.record("requestAnimationFrame", {
      details: { reason: "dom-focus-retry" },
    });
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
      const syncBase = this.getTraceFields(current, current.scopeId);
      const syncTransactionId = this.beginTrace("programmatic", null, syncBase);
      this.emitFocusCommit(
        syncTransactionId,
        syncBase,
        current,
        "programmatic",
        "dom-focus-sync",
        this.contexts.get(current.scopeId)?.generationId,
      );
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
    strategy = "unclassified",
    diagnostics?: {
      memoryGenerationId?: number;
      memoryDecision?: "accepted" | "rejected";
      memoryRejectionReason?: string;
    },
  ): void {
    useNavigationStore.getState().updateDebug({
      requestedDirection: direction,
      resolvedCandidate: candidate,
      evaluatedCandidates: evaluated,
      resolutionTimeMs: durationMs,
    });
    const trace = this.activeNavigationTrace;
    if (!trace) return;
    const targetEntry = trace.base.scopeId
      ? this.registry.get(candidate ?? "")
      : undefined;
    this.trace.emit("NAV_RESOLVE", trace.transactionId, {
      ...trace.base,
      inputSource: trace.inputSource,
      direction: trace.direction,
      route: this.getRoute(targetEntry),
      targetRegionId: targetEntry?.navigationRegion?.regionId ?? null,
      resolutionStrategy: strategy,
      candidatesConsidered: evaluated,
      selectedFocusId: candidate ?? null,
      selectedItemIndex: this.getItemIndex(targetEntry),
      focusReason: "directional-navigation",
      memoryGenerationId: diagnostics?.memoryGenerationId ?? null,
      memoryDecision: diagnostics?.memoryDecision ?? null,
      memoryRejectionReason: diagnostics?.memoryRejectionReason ?? null,
    });
  }
}
