import { useEffect, useState } from "react";
import { useProductStore } from "../../stores/product-store";
import { useNavigationStore } from "../../stores/navigation-store";
import { getPerformanceCounters } from "./performance-counters";

interface PerformanceMetrics {
  fps: number;
  averageFrameMs: number;
  worstFrameMs: number;
  overBudgetFrames: number;
  longTasks: number;
  mountedFocusables: number;
  mountedCards: number;
  appShellRenders: number;
  gameCardRenders: number;
}

const initialMetrics: PerformanceMetrics = {
  fps: 0,
  averageFrameMs: 0,
  worstFrameMs: 0,
  overBudgetFrames: 0,
  longTasks: 0,
  mountedFocusables: 0,
  mountedCards: 0,
  appShellRenders: 0,
  gameCardRenders: 0,
};

export function PerformanceOverlay() {
  const [metrics, setMetrics] = useState(initialMetrics);
  const [detailed, setDetailed] = useState(
    () =>
      typeof window !== "undefined" &&
      new URLSearchParams(window.location.search).get("hud") === "detail",
  );
  const activeView = useProductStore((state) => state.activeView);
  const inputMode = useNavigationStore((state) => state.inputMode);
  const navigationPhase = useNavigationStore((state) => state.navigationPhase);
  const activeFocusId = useNavigationStore((state) => state.activeFocusId);
  const debug = useNavigationStore((state) => state.debug);
  const enabled =
    !import.meta.env.PROD &&
    typeof window !== "undefined" &&
    (new URLSearchParams(window.location.search).has("hud") ||
      import.meta.env.VITE_PERFORMANCE_HUD === "true");

  useEffect(() => {
    if (!enabled) return;
    let frameHandle = 0;
    let lastFrame = performance.now();
    let sampleStarted = lastFrame;
    let frameCount = 0;
    let totalFrameMs = 0;
    let worstFrameMs = 0;
    let overBudgetFrames = 0;
    let longTasks = 0;
    const observer =
      typeof PerformanceObserver !== "undefined"
        ? new PerformanceObserver((list) => {
            longTasks += list.getEntries().length;
          })
        : null;
    try {
      observer?.observe({ entryTypes: ["longtask"] });
    } catch {
      // Long task entries are not available in every browser/WebView.
    }

    const measure = (now: number) => {
      const frameMs = now - lastFrame;
      lastFrame = now;
      frameCount += 1;
      totalFrameMs += frameMs;
      worstFrameMs = Math.max(worstFrameMs, frameMs);
      if (frameMs > 16.67) overBudgetFrames += 1;

      if (now - sampleStarted >= 500) {
        const elapsed = now - sampleStarted;
        const counters = getPerformanceCounters();
        setMetrics({
          fps: Math.round((frameCount * 1000) / elapsed),
          averageFrameMs: totalFrameMs / frameCount,
          worstFrameMs,
          overBudgetFrames,
          longTasks,
          mountedFocusables: document.querySelectorAll(
            '[data-focusable="true"]',
          ).length,
          mountedCards: document.querySelectorAll(".game-card").length,
          appShellRenders: counters.appShellRenders,
          gameCardRenders: counters.gameCardRenders,
        });
        sampleStarted = now;
        frameCount = 0;
        totalFrameMs = 0;
        worstFrameMs = 0;
        overBudgetFrames = 0;
        longTasks = 0;
      }
      frameHandle = window.requestAnimationFrame(measure);
    };

    frameHandle = window.requestAnimationFrame(measure);
    return () => {
      window.cancelAnimationFrame(frameHandle);
      observer?.disconnect();
    };
  }, [enabled]);

  if (!enabled) return null;

  return (
    <aside
      className={`performance-overlay${detailed ? " is-detailed" : ""}`}
      aria-label="Performance overlay"
    >
      <div className="performance-overlay-heading">
        <strong>Performance</strong>
        <button type="button" onClick={() => setDetailed((value) => !value)}>
          {detailed ? "compact" : "detail"}
        </button>
      </div>
      <span>{metrics.fps} FPS</span>
      <span>{metrics.averageFrameMs.toFixed(1)} ms avg</span>
      <span>{metrics.worstFrameMs.toFixed(1)} ms worst</span>
      {detailed && (
        <>
          <span>{metrics.overBudgetFrames} frames &gt; 16.67 ms</span>
          <span>{metrics.longTasks} long tasks</span>
          <span>
            {activeView} · {inputMode}
          </span>
          <span>
            {navigationPhase} · {activeFocusId ?? "—"}
          </span>
          <span>
            {metrics.mountedCards} cards / {metrics.mountedFocusables}{" "}
            focusables
          </span>
          <span>
            {metrics.appShellRenders} shell / {metrics.gameCardRenders} card
            renders
          </span>
          <span>
            bg #{debug.backgroundRequestId ?? "—"}{" "}
            {debug.backgroundStatus ?? "—"}
          </span>
          <span>
            cache {debug.backgroundCacheHits ?? 0}/
            {debug.backgroundCacheMisses ?? 0}
          </span>
          <span>
            decode {(debug.backgroundDecodeTimeMs ?? 0).toFixed(1)} ms
          </span>
          <span>transition {debug.transitionActive ? "active" : "idle"}</span>
        </>
      )}
    </aside>
  );
}
