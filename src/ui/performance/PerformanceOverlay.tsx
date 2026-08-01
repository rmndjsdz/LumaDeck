import { useEffect, useState } from "react";

interface PerformanceMetrics {
  fps: number;
  averageFrameMs: number;
  worstFrameMs: number;
  mountedFocusables: number;
  mountedCards: number;
}

const initialMetrics: PerformanceMetrics = {
  fps: 0,
  averageFrameMs: 0,
  worstFrameMs: 0,
  mountedFocusables: 0,
  mountedCards: 0,
};

export function PerformanceOverlay() {
  const [metrics, setMetrics] = useState(initialMetrics);

  useEffect(() => {
    if (import.meta.env.PROD) return;
    let frameHandle = 0;
    let lastFrame = performance.now();
    let sampleStarted = lastFrame;
    let frameCount = 0;
    let totalFrameMs = 0;
    let worstFrameMs = 0;

    const measure = (now: number) => {
      const frameMs = now - lastFrame;
      lastFrame = now;
      frameCount += 1;
      totalFrameMs += frameMs;
      worstFrameMs = Math.max(worstFrameMs, frameMs);

      if (now - sampleStarted >= 500) {
        const elapsed = now - sampleStarted;
        setMetrics({
          fps: Math.round((frameCount * 1000) / elapsed),
          averageFrameMs: totalFrameMs / frameCount,
          worstFrameMs,
          mountedFocusables: document.querySelectorAll(
            '[data-focusable="true"]',
          ).length,
          mountedCards: document.querySelectorAll(".game-card").length,
        });
        sampleStarted = now;
        frameCount = 0;
        totalFrameMs = 0;
        worstFrameMs = 0;
      }
      frameHandle = window.requestAnimationFrame(measure);
    };

    frameHandle = window.requestAnimationFrame(measure);
    return () => window.cancelAnimationFrame(frameHandle);
  }, []);

  if (import.meta.env.PROD) return null;

  return (
    <aside className="performance-overlay" aria-label="Performance overlay">
      <strong>Performance</strong>
      <span>{metrics.fps} FPS</span>
      <span>{metrics.averageFrameMs.toFixed(1)} ms avg</span>
      <span>{metrics.worstFrameMs.toFixed(1)} ms worst</span>
      <span>{metrics.mountedCards} cards mounted</span>
      <span>{metrics.mountedFocusables} focusables</span>
    </aside>
  );
}
