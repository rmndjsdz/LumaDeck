import { useLayoutEffect, useRef, type PropsWithChildren } from "react";

import { useNavigation } from "../navigation-context";

interface ScrollRestorationProps extends PropsWithChildren {
  scopeId: string;
  className?: string;
}

export function ScrollRestoration({
  scopeId,
  className,
  children,
}: ScrollRestorationProps) {
  const { scrollManager } = useNavigation();
  const containerRef = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    scrollManager.restore(scopeId, container);
    return () => scrollManager.remember(scopeId, container);
  }, [scopeId, scrollManager]);

  return (
    <div ref={containerRef} className={className} data-scroll-scope={scopeId}>
      {children}
    </div>
  );
}
