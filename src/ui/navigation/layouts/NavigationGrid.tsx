import { FocusScope } from "../focus/FocusScope";
import type { CSSProperties } from "react";
import type { NavigationLayoutProps } from "./layout-types";

interface NavigationGridProps extends NavigationLayoutProps {
  columns?: number;
}

export function NavigationGrid({
  scopeId,
  initialFocusId,
  restoreFocus,
  rememberScroll,
  className,
  columns = 5,
  children,
}: NavigationGridProps) {
  const content = (
    <div
      className={`navigation-grid ${className ?? ""}`}
      data-navigation-group="grid"
      style={
        {
          gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
        } satisfies CSSProperties
      }
    >
      {children}
    </div>
  );
  if (!scopeId) return content;
  return (
    <FocusScope
      scopeId={scopeId}
      initialFocusId={initialFocusId}
      restoreFocus={restoreFocus}
      rememberScroll={rememberScroll}
    >
      {content}
    </FocusScope>
  );
}
