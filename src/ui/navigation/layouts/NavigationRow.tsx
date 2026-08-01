import { useId, useMemo } from "react";

import { FocusScope } from "../focus/FocusScope";
import type { NavigationLayoutProps } from "./layout-types";
import {
  LinearNavigationContext,
  RowNavigationContext,
} from "./linear-navigation-context";
import { useRowNavigationGroup } from "./row-navigation-group-context";

interface NavigationRowProps extends NavigationLayoutProps {
  groupId?: string;
  rowId?: string;
  rowIndex?: number;
  orientation?: "horizontal";
  preserveHorizontalIntent?: boolean;
  wrap?: boolean;
}

export function NavigationRow({
  scopeId,
  initialFocusId,
  restoreFocus,
  rememberScroll,
  className,
  children,
  groupId,
  rowId,
  rowIndex,
  orientation = "horizontal",
  preserveHorizontalIntent,
  wrap = false,
}: NavigationRowProps) {
  const generatedGroupId = useId();
  const rowGroup = useRowNavigationGroup();
  const resolvedRowId = rowId ?? groupId;
  const resolvedGroupId = groupId ?? resolvedRowId ?? generatedGroupId;
  const rowNavigation =
    resolvedRowId !== undefined && rowIndex !== undefined
      ? {
          groupId: rowGroup?.groupId ?? resolvedGroupId,
          rowId: resolvedRowId,
          rowIndex,
          preserveHorizontalIntent:
            preserveHorizontalIntent ??
            rowGroup?.preserveHorizontalIntent ??
            false,
        }
      : null;
  const linearNavigation = useMemo(
    () => ({
      groupId: resolvedGroupId,
      axis: orientation,
      wrap,
    }),
    [orientation, resolvedGroupId, wrap],
  );
  const content = (
    <RowNavigationContext.Provider value={rowNavigation}>
      <LinearNavigationContext.Provider value={linearNavigation}>
        <div
          className={`navigation-row ${className ?? ""}`}
          data-navigation-group="row"
          data-linear-navigation={linearNavigation.groupId}
          data-row-id={resolvedRowId}
          data-row-index={rowIndex}
        >
          {children}
        </div>
      </LinearNavigationContext.Provider>
    </RowNavigationContext.Provider>
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
