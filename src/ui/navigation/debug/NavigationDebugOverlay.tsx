import { useState } from "react";
import { useNavigationStore } from "../../../stores/navigation-store";
import { navigationRuntimeTrace } from "./navigation-runtime-trace";

export function NavigationDebugOverlay() {
  const [traceStatus, setTraceStatus] = useState("ready");
  const inputMode = useNavigationStore((state) => state.inputMode);
  const navigationPhase = useNavigationStore((state) => state.navigationPhase);
  const activeScopeId = useNavigationStore((state) => state.activeScopeId);
  const activeFocusId = useNavigationStore((state) => state.activeFocusId);
  const previousFocusId = useNavigationStore((state) => state.previousFocusId);
  const lastNavigationAction = useNavigationStore(
    (state) => state.lastNavigationAction,
  );
  const debug = useNavigationStore((state) => state.debug);

  if (import.meta.env.PROD) return null;

  const copyTrace = async () => {
    const copied = await navigationRuntimeTrace.copyToClipboard();
    setTraceStatus(copied ? "copied" : "clipboard unavailable");
  };

  return (
    <aside className="debug-overlay" aria-label="Navigation debug overlay">
      <div className="debug-heading">
        <span className="debug-dot" />
        <strong>Navigation debug</strong>
      </div>
      <dl>
        <DebugRow label="inputMode" value={inputMode} />
        <DebugRow label="navigation phase" value={navigationPhase} />
        <DebugRow
          label="gamepad"
          value={debug.gamepadConnected ? "connected" : "none"}
        />
        {debug.gamepad && (
          <>
            <DebugRow label="pad id" value={debug.gamepad.id} />
            <DebugRow
              label="pad input"
              value={debug.gamepad.direction ?? "neutral"}
            />
            <DebugRow
              label="buttons"
              value={
                debug.gamepad.pressedButtons.length
                  ? debug.gamepad.pressedButtons.join(", ")
                  : "none"
              }
            />
          </>
        )}
        <DebugRow label="last action" value={lastNavigationAction ?? "—"} />
        <DebugRow label="scope" value={activeScopeId ?? "—"} />
        <DebugRow
          label="scope lifecycle"
          value={debug.scopeLifecycleState ?? "—"}
        />
        <DebugRow
          label="requested initial"
          value={debug.requestedInitialFocusId ?? "—"}
        />
        <DebugRow label="active focus" value={activeFocusId ?? "—"} />
        <DebugRow
          label="navigation level"
          value={debug.activeNavigationLevel ?? "—"}
        />
        <DebugRow label="main tab" value={debug.selectedMainTab ?? "—"} />
        <DebugRow
          label="hierarchy parent/child"
          value={`${debug.parentRegionId ?? "—"}/${debug.childRegionId ?? "—"}`}
        />
        <DebugRow
          label="hierarchy transition"
          value={debug.hierarchyTransitionReason ?? "—"}
        />
        <DebugRow
          label="hierarchy restore"
          value={debug.hierarchyRestoreReason ?? "—"}
        />
        <DebugRow
          label="last region focus"
          value={formatRegionFocus(debug.lastFocusedByRegion)}
        />
        <DebugRow
          label="active valid"
          value={
            debug.activeFocusValid === undefined
              ? "—"
              : String(debug.activeFocusValid)
          }
        />
        <DebugRow
          label="DOM focus"
          value={debug.domActiveElementFocusId ?? "—"}
        />
        <DebugRow
          label="scope registered"
          value={formatNumber(debug.registeredActiveScopeFocusables)}
        />
        <DebugRow label="previous focus" value={previousFocusId ?? "—"} />
        <DebugRow
          label="active index"
          value={formatNumber(debug.activeAbsoluteIndex)}
        />
        <DebugRow label="active row" value={formatNumber(debug.activeRow)} />
        <DebugRow
          label="active column"
          value={formatNumber(debug.activeColumn)}
        />
        <DebugRow
          label="target index"
          value={formatNumber(debug.targetAbsoluteIndex)}
        />
        <DebugRow label="target row" value={formatNumber(debug.targetRow)} />
        <DebugRow
          label="target column"
          value={formatNumber(debug.targetColumn)}
        />
        <DebugRow
          label="window"
          value={`${formatNumber(debug.windowStart)}–${formatNumber(debug.windowEnd)}`}
        />
        <DebugRow label="pending focus" value={debug.pendingFocusId ?? "—"} />
        <DebugRow
          label="pending request"
          value={formatNumber(debug.pendingRequestId)}
        />
        <DebugRow
          label="scope request"
          value={formatNumber(debug.pendingScopeActivationRequestId)}
        />
        <DebugRow
          label="canceled Library"
          value={formatNumber(debug.canceledLibraryRequestId)}
        />
        <DebugRow label="anchor" value={debug.anchorFocusId ?? "—"} />
        <DebugRow
          label="scroll before/after"
          value={`${formatNumber(debug.scrollTopBefore)}/${formatNumber(debug.scrollTopAfter)}`}
        />
        <DebugRow
          label="scroll authority"
          value={debug.scrollAuthority ?? "—"}
        />
        <DebugRow label="fallback" value={debug.fallbackReason ?? "—"} />
        <DebugRow label="Home row" value={debug.activeHomeRowId ?? "â€”"} />
        <DebugRow
          label="Home row/index"
          value={`${formatNumber(debug.activeHomeRowIndex)}/${formatNumber(debug.activeHomeItemIndex)}`}
        />
        <DebugRow
          label="Home preferred"
          value={`${formatNumber(debug.preferredHomeItemIndex)} @ ${formatNumber(debug.preferredHomeCenterX)}`}
        />
        <DebugRow
          label="Home target"
          value={`${debug.targetHomeRowId ?? "â€”"}/${formatNumber(debug.targetHomeItemIndex)}`}
        />
        <DebugRow
          label="vertical strategy"
          value={debug.selectedVerticalStrategy ?? "â€”"}
        />
        <DebugRow
          label="target items"
          value={debug.availableTargetRowItems?.join(", ") ?? "â€”"}
        />
        <DebugRow
          label="vertical fallback"
          value={debug.verticalFallbackReason ?? "â€”"}
        />
        <DebugRow
          label="restored row/index"
          value={`${formatNumber(debug.restoredHomeRowIndex)}/${formatNumber(debug.restoredHomeItemIndex)}`}
        />
        <DebugRow
          label="focus failure"
          value={debug.lastFocusFailureReason ?? "—"}
        />
        <DebugRow label="registered" value={String(debug.registryCount)} />
        <DebugRow label="direction" value={debug.requestedDirection ?? "—"} />
        <DebugRow label="candidate" value={debug.resolvedCandidate ?? "—"} />
        <DebugRow
          label="resolution"
          value={`${debug.resolutionTimeMs.toFixed(2)} ms`}
        />
        <DebugRow label="restored" value={debug.lastRestoredFocus ?? "—"} />
        <DebugRow label="scroll" value={debug.lastScroll ?? "—"} />
        <DebugRow label="actions/s" value={String(debug.actionsPerSecond)} />
        <DebugRow label="focus losses" value={String(debug.focusLosses)} />
        <DebugRow
          label="duplicates"
          value={
            debug.duplicateFocusIds.length
              ? debug.duplicateFocusIds.join(", ")
              : "—"
          }
        />
      </dl>
      <small>{debug.evaluatedCandidates.length} candidates evaluated</small>
      <div className="debug-trace-actions">
        <button type="button" onClick={() => void copyTrace()}>
          Copy navigation trace ({navigationRuntimeTrace.getCount()})
        </button>
        <button type="button" onClick={() => navigationRuntimeTrace.download()}>
          Export JSON
        </button>
        <button
          type="button"
          onClick={() => {
            navigationRuntimeTrace.clear();
            setTraceStatus("cleared");
          }}
        >
          Clear trace
        </button>
        <small aria-live="polite">{traceStatus}</small>
      </div>
    </aside>
  );
}

function DebugRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="debug-row">
      <dt>{label}</dt>
      <dd title={value}>{value}</dd>
    </div>
  );
}

function formatNumber(value: number | undefined): string {
  return value === undefined ? "—" : String(value);
}

function formatRegionFocus(value: Record<string, string> | undefined): string {
  if (!value) return "—";
  const entries = Object.entries(value);
  return entries.length
    ? entries.map(([regionId, focusId]) => `${regionId}:${focusId}`).join(", ")
    : "—";
}
