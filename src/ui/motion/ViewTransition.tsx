import type { CSSProperties, PropsWithChildren } from "react";
import { useProductStore, type ProductView } from "../../stores/product-store";
import { useNavigationStore } from "../../stores/navigation-store";
import {
  markPerformance,
  measurePerformance,
} from "../performance/performance-marks";
import { motionTokens } from "./motion-tokens";
import { navigationRuntimeTrace } from "../navigation/debug/navigation-runtime-trace";

export function ViewTransition({
  view,
  children,
}: PropsWithChildren<{ view: ProductView }>) {
  const transitionId = useProductStore((state) => state.viewTransitionId);
  const setTransitionActive = useNavigationStore((state) => state.updateDebug);
  return (
    <div
      className="view-transition"
      data-view={view}
      data-transition-id={transitionId}
      style={
        {
          "--view-enter-duration": `${motionTokens.duration.viewEnter}ms`,
          "--view-exit-duration": `${motionTokens.duration.viewExit}ms`,
        } as CSSProperties
      }
      onAnimationStart={() => {
        navigationRuntimeTrace.record("animation", {
          animationState: "running",
          transitionState: "running",
          details: { phase: "start", view },
        });
        markPerformance("view-active");
        setTransitionActive({ transitionActive: true });
      }}
      onAnimationEnd={() => {
        navigationRuntimeTrace.record("animation", {
          animationState: "idle",
          transitionState: "idle",
          details: { phase: "end", view },
        });
        markPerformance("view-active");
        measurePerformance("view-transition", "view-requested", "view-active");
        setTransitionActive({ transitionActive: false });
      }}
    >
      {children}
    </div>
  );
}
