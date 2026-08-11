import { useLayoutEffect, useRef, type PropsWithChildren } from "react";
import { motionTokens } from "../../ui/motion/motion-tokens";

type DetailsTabContentProps = PropsWithChildren<{
  activeSection: string;
  direction: "forward" | "backward";
}>;

export function DetailsTabContent({
  activeSection,
  direction,
  children,
}: DetailsTabContentProps) {
  const contentRef = useRef<HTMLDivElement | null>(null);

  useLayoutEffect(() => {
    const element = contentRef.current;
    if (!element || typeof element.animate !== "function") return;
    if (
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches
    ) {
      return;
    }

    const offset = direction === "forward" ? "22px" : "-22px";
    const animation = element.animate(
      [
        {
          opacity: 0,
          transform: `translate3d(${offset}, 0, 0)`,
        },
        {
          opacity: 1,
          transform: "translate3d(0, 0, 0)",
        },
      ],
      {
        duration: motionTokens.duration.standard,
        easing: motionTokens.easing.enter,
        fill: "both",
      },
    );

    return () => animation.cancel();
  }, [activeSection, direction]);

  return (
    <div
      ref={contentRef}
      className={`details-tab-content is-${direction}`}
      data-transition-direction={direction}
      data-active-section={activeSection}
    >
      {children}
    </div>
  );
}
