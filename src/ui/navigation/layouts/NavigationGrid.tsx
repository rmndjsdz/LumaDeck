import { useId, useMemo } from "react";

import { FocusScope } from "../focus/FocusScope";
import type { CSSProperties } from "react";
import type { NavigationLayoutProps } from "./layout-types";
import { GridNavigationContext } from "./linear-navigation-context";

interface NavigationGridProps extends NavigationLayoutProps {
  columns?: number;
  groupId?: string;
  itemCount?: number;
  onRequestIndex?: (index: number) => void;
  resolveFocusId?: (index: number) => string;
  style?: CSSProperties;
}

export function NavigationGrid({
  scopeId,
  initialFocusId,
  restoreFocus,
  rememberScroll,
  className,
  columns = 5,
  groupId,
  itemCount,
  onRequestIndex,
  resolveFocusId,
  style,
  children,
}: NavigationGridProps) {
  const generatedGroupId = useId();
  const gridNavigation = useMemo(
    () => ({
      groupId: groupId ?? `${scopeId ?? "grid"}-${generatedGroupId}`,
      columns,
      itemCount,
      onRequestIndex,
      resolveFocusId,
    }),
    [
      columns,
      generatedGroupId,
      groupId,
      itemCount,
      onRequestIndex,
      resolveFocusId,
      scopeId,
    ],
  );
  const content = (
    <GridNavigationContext.Provider value={gridNavigation}>
      <div
        className={`navigation-grid ${className ?? ""}`}
        data-navigation-group="grid"
        data-grid-columns={columns}
        data-grid-navigation={gridNavigation.groupId}
        style={
          {
            gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
            ...style,
          } satisfies CSSProperties
        }
      >
        {children}
      </div>
    </GridNavigationContext.Provider>
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
