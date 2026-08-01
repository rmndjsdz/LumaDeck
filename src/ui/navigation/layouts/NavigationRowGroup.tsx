import { useMemo, type PropsWithChildren } from "react";

import {
  RowNavigationGroupContext,
  type RowNavigationGroupContextValue,
} from "./row-navigation-group-context";

export interface NavigationRowGroupProps extends PropsWithChildren {
  scopeId: string;
  groupId?: string;
  orientation?: "vertical";
  preserveHorizontalIntent?: boolean;
  className?: string;
}

export function NavigationRowGroup({
  scopeId,
  groupId = scopeId,
  orientation = "vertical",
  preserveHorizontalIntent = false,
  className,
  children,
}: NavigationRowGroupProps) {
  const value = useMemo<RowNavigationGroupContextValue>(
    () => ({
      groupId,
      scopeId,
      orientation,
      preserveHorizontalIntent,
    }),
    [groupId, orientation, preserveHorizontalIntent, scopeId],
  );

  return (
    <RowNavigationGroupContext.Provider value={value}>
      <div
        className={className}
        data-navigation-row-group={groupId}
        data-navigation-row-group-scope={scopeId}
      >
        {children}
      </div>
    </RowNavigationGroupContext.Provider>
  );
}
