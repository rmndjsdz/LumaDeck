import type {
  FocusReason,
  InputSource,
  NavigationContext,
  NavigationDirection,
} from "./navigation-types";
import { navigationRuntimeTrace } from "../debug/navigation-runtime-trace";

export type NavigationTraceEventName =
  | "NAV_INPUT"
  | "NAV_RESOLVE"
  | "FOCUS_COMMIT"
  | "CONTEXT_SAVE"
  | "CONTEXT_RESTORE_BEGIN"
  | "CONTEXT_RESTORE_REQUEST_REUSED"
  | "NAV_INPUT_BLOCKED"
  | "CONTEXT_RESTORE_COMMIT"
  | "POINTER_SELECTION";

export interface NavigationTraceRecord {
  event: NavigationTraceEventName;
  transactionId: string;
  timestamp: number;
  inputSource: InputSource;
  direction: NavigationDirection | null;
  generationId: number | null;
  route: string;
  scopeId: string | null;
  fromRegionId: string | null;
  fromFocusId: string | null;
  fromItemIndex: number | null;
  preferredItemIndexBefore: number | null;
  targetRegionId: string | null;
  resolutionStrategy: string | null;
  candidatesConsidered: string[];
  selectedFocusId: string | null;
  selectedItemIndex: number | null;
  preferredItemIndexAfter: number | null;
  focusReason: FocusReason;
  memoryGenerationId: number | null;
  memoryDecision: "accepted" | "rejected" | null;
  memoryRejectionReason: string | null;
  pendingRestore: boolean;
  restoreOwner: string | null;
  restoreCommitCount: number;
  restoreRequestReuseReason: string | null;
  contextBefore: NavigationContext | null;
  contextAfter: NavigationContext | null;
}

export interface NavigationTraceOptions {
  enabled?: boolean;
  now?: () => number;
  log?: (record: NavigationTraceRecord) => void;
}

export class NavigationTrace {
  private readonly records: NavigationTraceRecord[] = [];
  private nextTransactionId = 0;
  private readonly enabled: boolean;
  private readonly now: () => number;
  private readonly log: (record: NavigationTraceRecord) => void;

  public constructor(options: NavigationTraceOptions = {}) {
    this.enabled = options.enabled ?? import.meta.env.DEV;
    this.now = options.now ?? (() => performance.now());
    this.log = options.log ?? (() => undefined);
  }

  public begin(
    inputSource: InputSource,
    direction: NavigationDirection | null,
    base: Omit<
      NavigationTraceRecord,
      "event" | "transactionId" | "timestamp" | "inputSource" | "direction"
    >,
  ): string {
    const transactionId = `nav-${++this.nextTransactionId}`;
    if (this.enabled) {
      this.emit("NAV_INPUT", transactionId, {
        ...base,
        inputSource,
        direction,
      });
    }
    return transactionId;
  }

  public emit(
    event: NavigationTraceEventName,
    transactionId: string,
    record: Omit<
      NavigationTraceRecord,
      | "event"
      | "transactionId"
      | "timestamp"
      | "generationId"
      | "memoryGenerationId"
      | "memoryDecision"
      | "memoryRejectionReason"
      | "pendingRestore"
      | "restoreOwner"
      | "restoreCommitCount"
      | "restoreRequestReuseReason"
      | "contextBefore"
      | "contextAfter"
    > &
      Partial<
        Pick<
          NavigationTraceRecord,
          | "generationId"
          | "memoryGenerationId"
          | "memoryDecision"
          | "memoryRejectionReason"
          | "pendingRestore"
          | "restoreOwner"
          | "restoreCommitCount"
          | "restoreRequestReuseReason"
          | "contextBefore"
          | "contextAfter"
        >
      >,
  ): void {
    if (!this.enabled) return;
    const complete: NavigationTraceRecord = {
      event,
      transactionId,
      timestamp: this.now(),
      ...record,
      generationId: record.generationId ?? null,
      memoryGenerationId: record.memoryGenerationId ?? null,
      memoryDecision: record.memoryDecision ?? null,
      memoryRejectionReason: record.memoryRejectionReason ?? null,
      pendingRestore: record.pendingRestore ?? false,
      restoreOwner: record.restoreOwner ?? null,
      restoreCommitCount: record.restoreCommitCount ?? 0,
      restoreRequestReuseReason: record.restoreRequestReuseReason ?? null,
      contextBefore: record.contextBefore ? { ...record.contextBefore } : null,
      contextAfter: record.contextAfter ? { ...record.contextAfter } : null,
      candidatesConsidered: [...record.candidatesConsidered],
    };
    this.records.push(complete);
    this.log(complete);
    navigationRuntimeTrace.recordNavigationTrace(complete);
  }

  public getRecords(): NavigationTraceRecord[] {
    return this.records.map((record) => ({
      ...record,
      contextBefore: record.contextBefore ? { ...record.contextBefore } : null,
      contextAfter: record.contextAfter ? { ...record.contextAfter } : null,
      candidatesConsidered: [...record.candidatesConsidered],
    }));
  }

  public clear(): void {
    this.records.length = 0;
  }
}
