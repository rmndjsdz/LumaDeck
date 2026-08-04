import { useNavigationStore } from "../../../stores/navigation-store";
import { useProductStore } from "../../../stores/product-store";
import type { FocusRegistry } from "../core/focus-registry";
import type { NavigationEngine } from "../core/navigation-engine";
import type { FocusReason, InputSource } from "../core/navigation-types";
import type { NavigationTraceRecord } from "../core/navigation-trace";

export type RuntimeTraceEvent =
  | "KEYDOWN_RECEIVED"
  | "KEYBOARD_ADAPTER_STARTED"
  | "KEYBOARD_ADAPTER_STOPPED"
  | "KEYBOARD_INTENT_CREATED"
  | "INPUT_ACCEPTED"
  | "INPUT_DISCARDED"
  | "NAVIGATION_REQUESTED"
  | "FOCUS_BEFORE"
  | "FOCUS_TARGET"
  | "FOCUS_AFTER"
  | "focus"
  | "blur"
  | "registerFocusable"
  | "unregisterFocusable"
  | "scope_register"
  | "scope_unregister"
  | "scope_active"
  | "scope_watchdog"
  | "fallback"
  | "route_transition"
  | "keyboard_open"
  | "keyboard_confirm"
  | "keyboard_close"
  | "result_set_change"
  | "library_content_change"
  | "library_mount"
  | "library_unmount"
  | "details_open"
  | "details_close"
  | "restore_cancel"
  | "animation"
  | "transition"
  | "resize_observer"
  | "mutation_observer"
  | "pointer"
  | "requestAnimationFrame"
  | "runtime_capabilities"
  | "NAV_INVARIANT_VIOLATION"
  | NavigationTraceRecord["event"];

type RuntimeTracePrimitive = string | number | boolean | null;

export interface LibraryRuntimeContent {
  queryVersion: number;
  queryLength: number;
  queryCommitted: boolean;
  filterIds: string[];
  sortId: string;
  resultCount: number;
  visibleResultIds: string[];
  resultGeneration: number;
  openerGameId: string | null;
  openerFocusId: string | null;
  openerPresentInResults: boolean | null;
}

export interface RuntimeTraceRecord {
  event: RuntimeTraceEvent;
  timestamp: number;
  sequence: number;
  route: string | null;
  screenLifecycle: string | null;
  activeScopeId: string | null;
  activeFocusId: string | null;
  previousFocusId: string | null;
  domActiveElementFocusId: string | null;
  domActiveElementTag: string | null;
  focusableRegistered: boolean;
  focusableVisible: boolean | null;
  focusableDisabled: boolean | null;
  focusableScopeId: string | null;
  regionId: string | null;
  gridId: string | null;
  rowId: string | null;
  itemIndex: number | null;
  absoluteIndex: number | null;
  generationId: number | null;
  transitionId: number | null;
  pendingRestore: boolean;
  pendingScopeActivation: boolean;
  restoreTarget: string | null;
  restoreOwner: string | null;
  focusReason: FocusReason | null;
  queryResultCount: number | null;
  queryVersion: number | null;
  queryLength: number | null;
  queryCommitted: boolean | null;
  filterIds: string[];
  sortId: string | null;
  resultCount: number | null;
  visibleResultIds: string[];
  resultGeneration: number | null;
  openerGameId: string | null;
  openerFocusId: string | null;
  openerPresentInResults: boolean | null;
  libraryLifecycle: "mounted" | "unmounted" | null;
  virtualKeyboardState: "closed" | "open" | "confirming" | "closing";
  animationState: "idle" | "running";
  transitionState: "idle" | "running";
  inputSettlingState: string;
  inputSource: InputSource | null;
  details: Record<string, RuntimeTracePrimitive>;
}

interface RuntimeTracePatch {
  activeFocusId?: string | null;
  focusReason?: FocusReason | null;
  generationId?: number | null;
  pendingRestore?: boolean;
  pendingScopeActivation?: boolean;
  restoreTarget?: string | null;
  restoreOwner?: string | null;
  queryResultCount?: number | null;
  libraryState?: LibraryRuntimeContent;
  openerGameId?: string | null;
  openerFocusId?: string | null;
  libraryLifecycle?: RuntimeTraceRecord["libraryLifecycle"];
  virtualKeyboardState?: RuntimeTraceRecord["virtualKeyboardState"];
  animationState?: RuntimeTraceRecord["animationState"];
  transitionState?: RuntimeTraceRecord["transitionState"];
  inputSource?: InputSource | null;
  details?: Record<string, RuntimeTracePrimitive>;
}

interface RuntimeAttachment {
  registry: FocusRegistry;
  engine: NavigationEngine;
}

const MAX_RECORDS = 4096;

class NavigationRuntimeTrace {
  private readonly records: RuntimeTraceRecord[] = [];
  private attachment: RuntimeAttachment | null = null;
  private sequence = 0;
  private virtualKeyboardState: RuntimeTraceRecord["virtualKeyboardState"] =
    "closed";
  private animationState: RuntimeTraceRecord["animationState"] = "idle";
  private transitionState: RuntimeTraceRecord["transitionState"] = "idle";
  private queryResultCount: number | null = null;
  private libraryContent: LibraryRuntimeContent | null = null;
  private libraryResultIds: string[] = [];
  private openerGameId: string | null = null;
  private openerFocusId: string | null = null;
  private pendingRestore = false;
  private restoreTarget: string | null = null;
  private restoreOwner: string | null = null;
  private observerCleanup: (() => void) | null = null;
  private recordingInvariant = false;

  public attach(attachment: RuntimeAttachment): void {
    this.attachment = attachment;
  }

  public startObservers(): () => void {
    if (this.observerCleanup || import.meta.env.PROD) {
      return () => undefined;
    }
    const focusIn = (event: Event) =>
      this.record("focus", {
        activeFocusId: this.getElementFocusId(event.target),
        details: { nativeEvent: "focusin" },
      });
    const focusOut = (event: Event) =>
      this.record("blur", {
        activeFocusId: this.getElementFocusId(event.target),
        details: { nativeEvent: "focusout" },
      });
    const animationStart = (event: Event) => {
      this.animationState = "running";
      this.record("animation", {
        animationState: "running",
        details: { phase: "start", nativeEvent: event.type },
      });
    };
    const animationEnd = (event: Event) => {
      this.animationState = "idle";
      this.record("animation", {
        animationState: "idle",
        details: { phase: "end", nativeEvent: event.type },
      });
    };
    const transitionStart = (event: Event) => {
      this.transitionState = "running";
      this.record("transition", {
        transitionState: "running",
        details: { phase: "start", nativeEvent: event.type },
      });
    };
    const transitionEnd = (event: Event) => {
      this.transitionState = "idle";
      this.record("transition", {
        transitionState: "idle",
        details: { phase: "end", nativeEvent: event.type },
      });
    };
    const pointer = (event: Event) => {
      const target = event.target instanceof HTMLElement ? event.target : null;
      this.record("pointer", {
        details: {
          phase: event.type,
          focusId: target?.dataset.focusId ?? null,
          pointerCaptureSupported:
            typeof target?.hasPointerCapture === "function",
        },
      });
    };
    const keydown = (event: KeyboardEvent) => {
      if (!event.key.startsWith("Arrow")) return;
      const target = event.target instanceof HTMLElement ? event.target : null;
      this.record("KEYDOWN_RECEIVED", {
        details: {
          key: event.key,
          targetTag: target?.tagName.toLowerCase() ?? null,
          targetFocusId: target?.dataset.focusId ?? null,
          defaultPrevented: event.defaultPrevented,
        },
      });
    };
    const listeners: Array<[keyof DocumentEventMap, EventListener]> = [
      ["focusin", focusIn],
      ["focusout", focusOut],
      ["animationstart", animationStart],
      ["animationend", animationEnd],
      ["animationcancel", animationEnd],
      ["transitionstart", transitionStart],
      ["transitionend", transitionEnd],
      ["transitioncancel", transitionEnd],
      ["pointerdown", pointer],
      ["pointerup", pointer],
      ["pointercancel", pointer],
    ];
    for (const [type, listener] of listeners) {
      document.addEventListener(type, listener);
    }
    window.addEventListener("keydown", keydown, true);

    const mutationObserver =
      typeof MutationObserver === "undefined"
        ? null
        : new MutationObserver((mutations) => {
            this.record("mutation_observer", {
              details: {
                records: mutations.length,
                childList: mutations.filter(
                  (mutation) => mutation.type === "childList",
                ).length,
                attributes: mutations.filter(
                  (mutation) => mutation.type === "attributes",
                ).length,
              },
            });
          });
    mutationObserver?.observe(document.body, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: [
        "data-active",
        "data-focus-id",
        "data-focus-scope",
        "data-view",
      ],
    });

    this.record("runtime_capabilities", {
      details: {
        requestAnimationFrame:
          typeof window.requestAnimationFrame === "function",
        getBoundingClientRect:
          typeof Element !== "undefined" &&
          typeof Element.prototype.getBoundingClientRect === "function",
        intersectionObserver: typeof IntersectionObserver !== "undefined",
        resizeObserver: typeof ResizeObserver !== "undefined",
        mutationObserver: typeof MutationObserver !== "undefined",
        pointerCapture:
          typeof Element !== "undefined" &&
          typeof Element.prototype.hasPointerCapture === "function",
        gamepadPolling: true,
        reactTransitions: false,
        tauriWindowLifecycle: Reflect.has(window, "__TAURI_INTERNALS__"),
      },
    });

    this.observerCleanup = () => {
      for (const [type, listener] of listeners) {
        document.removeEventListener(type, listener);
      }
      window.removeEventListener("keydown", keydown, true);
      mutationObserver?.disconnect();
      this.observerCleanup = null;
    };
    return this.observerCleanup;
  }

  public record(event: RuntimeTraceEvent, patch: RuntimeTracePatch = {}): void {
    if (import.meta.env.PROD) return;
    if (patch.virtualKeyboardState) {
      this.virtualKeyboardState = patch.virtualKeyboardState;
    }
    if (patch.animationState) this.animationState = patch.animationState;
    if (patch.transitionState) this.transitionState = patch.transitionState;
    if (patch.queryResultCount !== undefined) {
      this.queryResultCount = patch.queryResultCount;
    }
    if (patch.pendingRestore !== undefined) {
      this.pendingRestore = patch.pendingRestore;
    }
    if (patch.restoreTarget !== undefined) {
      this.restoreTarget = patch.restoreTarget;
    }
    if (patch.restoreOwner !== undefined) {
      this.restoreOwner = patch.restoreOwner;
    }
    if (patch.openerGameId !== undefined)
      this.openerGameId = patch.openerGameId;
    if (patch.openerFocusId !== undefined)
      this.openerFocusId = patch.openerFocusId;
    if (patch.openerGameId !== undefined || patch.openerFocusId !== undefined) {
      if (this.libraryContent) {
        this.libraryContent = {
          ...this.libraryContent,
          openerGameId: this.openerGameId,
          openerFocusId: this.openerFocusId,
          openerPresentInResults: this.openerGameId
            ? this.libraryResultIds.includes(this.openerGameId)
            : null,
        };
      }
    }
    if (patch.libraryState) this.libraryContent = patch.libraryState;

    const state = useNavigationStore.getState();
    const product = useProductStore.getState();
    const activeFocusId = patch.activeFocusId ?? state.activeFocusId;
    const entry = activeFocusId
      ? this.attachment?.registry.get(activeFocusId)
      : undefined;
    const domActiveElement = document.activeElement as HTMLElement | null;
    const activeScopeId = state.activeScopeId;
    const screenLifecycle = activeScopeId
      ? (this.attachment?.engine.getScopeLifecycleState(activeScopeId) ?? null)
      : null;
    const record: RuntimeTraceRecord = {
      event,
      timestamp: performance.now(),
      sequence: ++this.sequence,
      route:
        document.querySelector<HTMLElement>("[data-view]")?.dataset.view ??
        product.activeView,
      screenLifecycle: screenLifecycle as string | null,
      activeScopeId,
      activeFocusId,
      previousFocusId: state.previousFocusId,
      domActiveElementFocusId: domActiveElement?.dataset.focusId ?? null,
      domActiveElementTag: domActiveElement?.tagName.toLowerCase() ?? null,
      focusableRegistered: Boolean(entry),
      focusableVisible: entry ? this.isVisible(entry.element) : null,
      focusableDisabled: entry?.disabled ?? null,
      focusableScopeId: entry?.scopeId ?? null,
      regionId: entry?.navigationRegion?.regionId ?? null,
      gridId: entry?.gridNavigation?.groupId ?? null,
      rowId: entry?.rowNavigation?.rowId ?? null,
      itemIndex:
        entry?.rowNavigation?.itemIndex ?? entry?.gridNavigation?.index ?? null,
      absoluteIndex:
        entry?.rowNavigation?.itemIndex ?? entry?.gridNavigation?.index ?? null,
      generationId: patch.generationId ?? null,
      transitionId: product.viewTransitionId,
      pendingRestore: patch.pendingRestore ?? this.pendingRestore,
      pendingScopeActivation:
        patch.pendingScopeActivation ??
        state.debug.pendingScopeActivationRequestId !== undefined,
      restoreTarget: patch.restoreTarget ?? this.restoreTarget,
      restoreOwner: patch.restoreOwner ?? this.restoreOwner,
      focusReason: patch.focusReason ?? null,
      queryResultCount: patch.queryResultCount ?? this.queryResultCount,
      queryVersion: this.libraryContent?.queryVersion ?? null,
      queryLength: this.libraryContent?.queryLength ?? null,
      queryCommitted: this.libraryContent?.queryCommitted ?? null,
      filterIds: this.libraryContent?.filterIds ?? [],
      sortId: this.libraryContent?.sortId ?? null,
      resultCount: this.libraryContent?.resultCount ?? null,
      visibleResultIds: this.libraryContent?.visibleResultIds ?? [],
      resultGeneration: this.libraryContent?.resultGeneration ?? null,
      openerGameId: this.openerGameId,
      openerFocusId: this.openerFocusId,
      openerPresentInResults:
        this.libraryContent?.openerPresentInResults ?? null,
      libraryLifecycle: patch.libraryLifecycle ?? null,
      virtualKeyboardState:
        patch.virtualKeyboardState ?? this.virtualKeyboardState,
      animationState: patch.animationState ?? this.animationState,
      transitionState: patch.transitionState ?? this.transitionState,
      inputSettlingState: state.navigationPhase,
      inputSource: patch.inputSource ?? null,
      details: { ...(patch.details ?? {}) },
    };
    this.records.push(record);
    if (this.records.length > MAX_RECORDS) this.records.shift();
    if (
      event === "KEYDOWN_RECEIVED" ||
      event === "KEYBOARD_ADAPTER_STARTED" ||
      event === "KEYBOARD_ADAPTER_STOPPED" ||
      event === "KEYBOARD_INTENT_CREATED" ||
      event === "INPUT_ACCEPTED" ||
      event === "INPUT_DISCARDED" ||
      event === "NAVIGATION_REQUESTED" ||
      event === "FOCUS_BEFORE" ||
      event === "FOCUS_TARGET" ||
      event === "FOCUS_AFTER"
    ) {
      console.debug(
        `[LumaDeck navigation] ${event} ${JSON.stringify(record.details)}`,
      );
    }
    this.checkInvariants(record);
  }

  public recordNavigationTrace(trace: NavigationTraceRecord): void {
    const isRestoreEvent =
      trace.event === "CONTEXT_RESTORE_BEGIN" ||
      trace.event === "CONTEXT_RESTORE_COMMIT" ||
      trace.event === "CONTEXT_RESTORE_REQUEST_REUSED" ||
      trace.event === "NAV_INPUT_BLOCKED";
    if (trace.event === "CONTEXT_RESTORE_BEGIN") {
      this.pendingRestore = true;
      this.restoreTarget = trace.selectedFocusId;
      this.restoreOwner = trace.restoreOwner;
    }
    if (trace.event === "CONTEXT_RESTORE_COMMIT") {
      this.pendingRestore = false;
      this.restoreTarget = trace.selectedFocusId;
      this.restoreOwner = trace.restoreOwner;
    }
    const patch: RuntimeTracePatch = {
      focusReason: trace.focusReason,
      generationId: trace.generationId,
      inputSource: trace.inputSource,
      details: {
        transactionId: trace.transactionId,
        direction: trace.direction,
        fromFocusId: trace.fromFocusId,
        selectedFocusId: trace.selectedFocusId,
        resolutionStrategy: trace.resolutionStrategy,
        restoreCommitCount: trace.restoreCommitCount,
      },
    };
    if (trace.event === "NAV_INPUT") {
      this.record("NAVIGATION_REQUESTED", {
        inputSource: trace.inputSource,
        details: {
          direction: trace.direction,
          fromFocusId: trace.fromFocusId,
          scopeId: trace.scopeId,
        },
      });
      this.record("FOCUS_BEFORE", {
        inputSource: trace.inputSource,
        activeFocusId: trace.fromFocusId,
        details: {
          direction: trace.direction,
          registered: trace.fromFocusId
            ? Boolean(this.attachment?.registry.get(trace.fromFocusId))
            : false,
        },
      });
    }
    if (trace.event === "NAV_RESOLVE") {
      this.record("FOCUS_TARGET", {
        inputSource: trace.inputSource,
        activeFocusId: trace.selectedFocusId,
        details: {
          direction: trace.direction,
          resolutionStrategy: trace.resolutionStrategy,
          selectedFocusId: trace.selectedFocusId,
          candidates: trace.candidatesConsidered.length,
        },
      });
    }
    if (trace.event === "FOCUS_COMMIT") {
      this.record("FOCUS_AFTER", {
        inputSource: trace.inputSource,
        activeFocusId: trace.selectedFocusId,
        details: {
          focusReason: trace.focusReason,
          selectedFocusId: trace.selectedFocusId,
          domFocusId:
            document.activeElement instanceof HTMLElement
              ? (document.activeElement.dataset.focusId ?? null)
              : null,
        },
      });
    }
    if (trace.event === "NAV_INPUT_BLOCKED") {
      this.record("INPUT_DISCARDED", {
        inputSource: trace.inputSource,
        details: {
          reason: trace.resolutionStrategy ?? "navigation-engine-blocked",
          direction: trace.direction,
          pendingRestore: trace.pendingRestore,
        },
      });
    }
    if (isRestoreEvent) {
      patch.pendingRestore = trace.pendingRestore;
      patch.restoreOwner = trace.restoreOwner;
      patch.restoreTarget = trace.selectedFocusId;
    }
    this.record(trace.event, patch);
  }

  public setKeyboardState(
    state: RuntimeTraceRecord["virtualKeyboardState"],
  ): void {
    this.record(
      state === "open"
        ? "keyboard_open"
        : state === "confirming"
          ? "keyboard_confirm"
          : "keyboard_close",
      { virtualKeyboardState: state },
    );
  }

  public recordLibraryContent(
    content: Omit<
      LibraryRuntimeContent,
      "openerGameId" | "openerFocusId" | "openerPresentInResults"
    > & {
      resultIds: string[];
    },
  ): void {
    this.libraryResultIds = [...content.resultIds];
    this.libraryContent = {
      ...content,
      openerGameId: this.openerGameId,
      openerFocusId: this.openerFocusId,
      openerPresentInResults: this.openerGameId
        ? content.resultIds.includes(this.openerGameId)
        : null,
    };
    this.record("library_content_change", {
      libraryState: this.libraryContent,
      queryResultCount: content.resultCount,
    });
  }

  public setOpener(gameId: string | null, focusId: string | null): void {
    this.openerGameId = gameId;
    this.openerFocusId = focusId;
    if (this.libraryContent) {
      this.libraryContent = {
        ...this.libraryContent,
        openerGameId: gameId,
        openerFocusId: focusId,
        openerPresentInResults: gameId
          ? this.libraryResultIds.includes(gameId)
          : null,
      };
    }
  }

  public recordLibraryLifecycle(
    lifecycle: RuntimeTraceRecord["libraryLifecycle"],
  ): void {
    this.record(lifecycle === "mounted" ? "library_mount" : "library_unmount", {
      libraryLifecycle: lifecycle,
    });
  }

  public getRecords(): RuntimeTraceRecord[] {
    return this.records.map((record) => ({
      ...record,
      details: { ...record.details },
    }));
  }

  public getCount(): number {
    return this.records.length;
  }

  public clear(): void {
    this.records.length = 0;
    this.sequence = 0;
  }

  public toJson(): string {
    return JSON.stringify(
      {
        format: "lumadeck-navigation-runtime-trace-v1",
        generatedAt: new Date().toISOString(),
        records: this.getRecords(),
      },
      null,
      2,
    );
  }

  public async copyToClipboard(): Promise<boolean> {
    if (!navigator.clipboard?.writeText) return false;
    try {
      await navigator.clipboard.writeText(this.toJson());
      return true;
    } catch {
      return false;
    }
  }

  public download(): void {
    const blob = new Blob([this.toJson()], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `lumadeck-navigation-trace-${Date.now()}.json`;
    link.click();
    URL.revokeObjectURL(url);
  }

  private getElementFocusId(target: EventTarget | null): string | null {
    return target instanceof HTMLElement
      ? (target.dataset.focusId ?? null)
      : null;
  }

  private isVisible(element: HTMLElement): boolean {
    const style = getComputedStyle(element);
    return (
      element.isConnected &&
      style.display !== "none" &&
      style.visibility !== "hidden" &&
      style.opacity !== "0"
    );
  }

  private checkInvariants(record: RuntimeTraceRecord): void {
    if (this.recordingInvariant || record.event === "NAV_INVARIANT_VIOLATION") {
      return;
    }
    const violations: string[] = [];
    const state = useNavigationStore.getState();
    const entry = state.activeFocusId
      ? this.attachment?.registry.get(state.activeFocusId)
      : undefined;
    if (state.activeFocusId && !entry)
      violations.push("active-focus-unregistered");
    if (entry && state.activeScopeId && entry.scopeId !== state.activeScopeId) {
      violations.push("active-focus-outside-active-scope");
    }
    if (
      document.querySelector("[data-view]") &&
      state.activeScopeId &&
      !state.activeFocusId
    ) {
      violations.push("visible-route-without-active-focus");
    }
    if (
      record.event === "FOCUS_COMMIT" &&
      state.activeFocusId &&
      record.domActiveElementFocusId !== state.activeFocusId
    ) {
      violations.push("dom-focus-diverges-after-commit");
    }
    if (
      record.event === "CONTEXT_RESTORE_COMMIT" &&
      record.restoreTarget &&
      !this.attachment?.registry.get(record.restoreTarget)
    ) {
      violations.push("restore-committed-with-unregistered-target");
    }
    if (
      record.event === "unregisterFocusable" &&
      record.details.focusId === state.activeFocusId
    ) {
      violations.push("active-focus-unregistered-during-unregister");
    }
    if (record.event === "scope_active" && record.pendingRestore) {
      violations.push("scope-active-with-pending-restore");
    }
    if (
      record.event === "NAV_INPUT_BLOCKED" &&
      !record.pendingRestore &&
      record.details.direction !== null
    ) {
      violations.push("first-navigation-input-blocked-without-restore");
    }
    for (const violation of violations) {
      this.recordingInvariant = true;
      this.record("NAV_INVARIANT_VIOLATION", {
        details: { violation, sourceEvent: record.event },
      });
      this.recordingInvariant = false;
    }
  }
}

export const navigationRuntimeTrace = new NavigationRuntimeTrace();
