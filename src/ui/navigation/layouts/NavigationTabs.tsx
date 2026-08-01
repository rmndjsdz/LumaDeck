import { useId, useMemo } from "react";

import { FocusScope } from "../focus/FocusScope";
import type { NavigationLayoutProps } from "./layout-types";
import { LinearNavigationContext } from "./linear-navigation-context";

interface NavigationTabsProps extends NavigationLayoutProps {
  groupId?: string;
  wrap?: boolean;
}

export function NavigationTabs({
  scopeId,
  initialFocusId,
  restoreFocus,
  rememberScroll,
  className,
  children,
  groupId,
  wrap = false,
}: NavigationTabsProps) {
  const generatedGroupId = useId();
  const linearNavigation = useMemo(
    () => ({
      groupId: groupId ?? `${scopeId ?? "tabs"}-${generatedGroupId}`,
      axis: "horizontal" as const,
      wrap,
    }),
    [generatedGroupId, groupId, scopeId, wrap],
  );
  const content = (
    <LinearNavigationContext.Provider value={linearNavigation}>
      <nav
        className={`navigation-tabs ${className ?? ""}`}
        aria-label="Sections"
        data-linear-navigation={linearNavigation.groupId}
      >
        {children}
      </nav>
    </LinearNavigationContext.Provider>
  );
  if (!scopeId) return content;
  return (
    <FocusScope
      scopeId={scopeId}
      initialFocusId={initialFocusId}
      restoreFocus={restoreFocus}
      rememberScroll={rememberScroll}
      activateOnMount
    >
      {content}
    </FocusScope>
  );
}
