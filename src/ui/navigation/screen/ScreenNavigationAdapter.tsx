import { type CSSProperties, type PropsWithChildren } from "react";

import { FocusScope } from "../focus/FocusScope";
import type { NavigationScreenDefinition } from "./navigation-screen-contract";
import { useNavigationScreen } from "./useNavigationScreen";

export interface ScreenNavigationAdapterProps extends PropsWithChildren {
  definition: NavigationScreenDefinition;
  active?: boolean;
  className?: string;
  style?: CSSProperties;
}

export function ScreenNavigationAdapter({
  definition,
  active = true,
  className,
  style,
  children,
}: ScreenNavigationAdapterProps) {
  useNavigationScreen(definition, active);

  return (
    <FocusScope
      scopeId={definition.rootScope.scopeId}
      parentScopeId={definition.rootScope.parentScopeId}
      initialFocusId={definition.initialFocus}
      restoreFocus={definition.restorePolicy.restoreFocus}
      rememberScroll={definition.restorePolicy.rememberScroll}
      activateOnMount
      onBack={definition.onBack}
      onAction={definition.onAction}
      className={className}
      style={style}
    >
      {children}
    </FocusScope>
  );
}
