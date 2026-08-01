import { useId, useMemo } from "react";

import { FocusScope } from "../focus/FocusScope";
import type { NavigationLayoutProps } from "./layout-types";
import { LinearNavigationContext } from "./linear-navigation-context";

interface NavigationRowProps extends NavigationLayoutProps {
  groupId?: string;
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
  wrap = false,
}: NavigationRowProps) {
  const generatedGroupId = useId();
  const linearNavigation = useMemo(
    () => ({
      groupId: groupId ?? `${scopeId ?? "row"}-${generatedGroupId}`,
      axis: "horizontal" as const,
      wrap,
    }),
    [generatedGroupId, groupId, scopeId, wrap],
  );
  const content = (
    <LinearNavigationContext.Provider value={linearNavigation}>
      <div
        className={`navigation-row ${className ?? ""}`}
        data-navigation-group="row"
        data-linear-navigation={linearNavigation.groupId}
      >
        {children}
      </div>
    </LinearNavigationContext.Provider>
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
