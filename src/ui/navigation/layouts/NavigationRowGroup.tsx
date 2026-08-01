import { useMemo, type PropsWithChildren } from "react";

import {
  RowNavigationGroupContext,
  type RowNavigationGroupContextValue,
} from "./row-navigation-group-context";
import { NavigationRegionContext } from "./linear-navigation-context";
import type { NavigationRegionConfig } from "../core/navigation-hierarchy";

export interface NavigationRowGroupProps extends PropsWithChildren {
  scopeId: string;
  groupId?: string;
  orientation?: "vertical";
  preserveHorizontalIntent?: boolean;
  regionId?: string;
  parentRegionId?: string;
  entryFocusId?: string;
  exitFocusId?: string;
  className?: string;
}

export function NavigationRowGroup({
  scopeId,
  groupId = scopeId,
  orientation = "vertical",
  preserveHorizontalIntent = false,
  regionId,
  parentRegionId,
  entryFocusId,
  exitFocusId,
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
  const regionValue = useMemo<NavigationRegionConfig | null>(
    () =>
      regionId ? { regionId, parentRegionId, entryFocusId, exitFocusId } : null,
    [entryFocusId, exitFocusId, parentRegionId, regionId],
  );

  return (
    <RowNavigationGroupContext.Provider value={value}>
      <NavigationRegionContext.Provider value={regionValue}>
        <div
          className={className}
          data-navigation-row-group={groupId}
          data-navigation-row-group-scope={scopeId}
        >
          {children}
        </div>
      </NavigationRegionContext.Provider>
    </RowNavigationGroupContext.Provider>
  );
}
