import { useNavigationStore } from "../../../stores/navigation-store";

export function NavigationDebugOverlay() {
  const inputMode = useNavigationStore((state) => state.inputMode);
  const activeScopeId = useNavigationStore((state) => state.activeScopeId);
  const activeFocusId = useNavigationStore((state) => state.activeFocusId);
  const previousFocusId = useNavigationStore((state) => state.previousFocusId);
  const lastNavigationAction = useNavigationStore(
    (state) => state.lastNavigationAction,
  );
  const debug = useNavigationStore((state) => state.debug);

  if (import.meta.env.PROD) return null;

  return (
    <aside className="debug-overlay" aria-label="Navigation debug overlay">
      <div className="debug-heading">
        <span className="debug-dot" />
        <strong>Navigation debug</strong>
      </div>
      <dl>
        <DebugRow label="inputMode" value={inputMode} />
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
        <DebugRow label="active focus" value={activeFocusId ?? "—"} />
        <DebugRow label="previous focus" value={previousFocusId ?? "—"} />
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
