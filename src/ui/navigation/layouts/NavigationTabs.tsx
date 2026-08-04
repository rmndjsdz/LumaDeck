import { createContext, useContext, useId, useMemo } from "react";

import { FocusScope } from "../focus/FocusScope";
import { Focusable, type FocusableProps } from "../focus/Focusable";
import {
  LinearNavigationContext,
  NavigationRegionContext,
} from "./linear-navigation-context";
import type { NavigationLayoutProps } from "./layout-types";
import type { NavigationRegionConfig } from "../core/navigation-hierarchy";

interface NavigationTabsContextValue {
  selectedId?: string;
  onSelect?: (focusId: string) => void;
  activationMode: NavigationTabsActivationMode;
  upTargetId?: string;
}

const NavigationTabsContext = createContext<NavigationTabsContextValue | null>(
  null,
);

export type NavigationTabsActivationMode = "automatic" | "manual";

interface NavigationTabsProps extends NavigationLayoutProps {
  groupId?: string;
  wrap?: boolean;
  selectedId?: string;
  onSelect?: (focusId: string) => void;
  activationMode?: NavigationTabsActivationMode;
  upTargetId?: string;
  navigationRegion?: NavigationRegionConfig;
  ariaLabel?: string;
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
  selectedId,
  onSelect,
  activationMode = "manual",
  upTargetId,
  navigationRegion,
  ariaLabel = "Sections",
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
    <NavigationTabsContext.Provider
      value={{ selectedId, onSelect, activationMode, upTargetId }}
    >
      <NavigationRegionContext.Provider value={navigationRegion ?? null}>
        <LinearNavigationContext.Provider value={linearNavigation}>
          <nav
            className={`navigation-tabs ${className ?? ""}`}
            aria-label={ariaLabel}
            role="tablist"
            aria-orientation="horizontal"
            data-linear-navigation={linearNavigation.groupId}
          >
            {children}
          </nav>
        </LinearNavigationContext.Provider>
      </NavigationRegionContext.Provider>
    </NavigationTabsContext.Provider>
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

export interface NavigationTabProps extends Omit<
  FocusableProps,
  "ariaSelected" | "onConfirm"
> {
  onConfirm?: () => void;
}

export function NavigationTab({
  onConfirm,
  onFocus,
  navigation,
  ...props
}: NavigationTabProps) {
  const context = useContext(NavigationTabsContext);
  const tabNavigation = context?.upTargetId
    ? { up: context.upTargetId, ...navigation }
    : navigation;
  return (
    <Focusable
      {...props}
      role="tab"
      navigation={tabNavigation}
      ariaSelected={context?.selectedId === props.focusId}
      onFocus={() => {
        if (context?.activationMode === "automatic") {
          context.onSelect?.(props.focusId);
        }
        onFocus?.();
      }}
      onConfirm={() => {
        context?.onSelect?.(props.focusId);
        onConfirm?.();
      }}
    />
  );
}
