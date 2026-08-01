import { useId, useMemo } from "react";

import { FocusScope } from "../focus/FocusScope";
import type { CSSProperties } from "react";
import type { NavigationLayoutProps } from "./layout-types";
import { GridNavigationContext } from "./linear-navigation-context";

interface NavigationGridProps extends NavigationLayoutProps {
  columns?: number;
  groupId?: string;
}

export function NavigationGrid({
  scopeId,
  initialFocusId,
  restoreFocus,
  rememberScroll,
  className,
  columns = 5,
  groupId,
  children,
}: NavigationGridProps) {
  const generatedGroupId = useId();
  const gridNavigation = useMemo(
    () => ({
      groupId: groupId ?? `${scopeId ?? "grid"}-${generatedGroupId}`,
      columns,
    }),
    [columns, generatedGroupId, groupId, scopeId],
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
