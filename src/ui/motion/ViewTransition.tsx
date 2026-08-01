import type { CSSProperties, PropsWithChildren } from "react";
import { useProductStore, type ProductView } from "../../stores/product-store";
import { useNavigationStore } from "../../stores/navigation-store";
import {
  markPerformance,
  measurePerformance,
} from "../performance/performance-marks";
import { motionTokens } from "./motion-tokens";

export function ViewTransition({
  view,
  children,
}: PropsWithChildren<{ view: ProductView }>) {
  const transitionId = useProductStore((state) => state.viewTransitionId);
  const setTransitionActive = useNavigationStore((state) => state.updateDebug);
  return (
    <div
      key={`${view}-${transitionId}`}
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
        markPerformance("view-active");
        setTransitionActive({ transitionActive: true });
      }}
      onAnimationEnd={() => {
        markPerformance("view-active");
        measurePerformance("view-transition", "view-requested", "view-active");
        setTransitionActive({ transitionActive: false });
      }}
    >
      {children}
    </div>
  );
}
