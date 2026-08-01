import type { PropsWithChildren } from "react";

export interface NavigationLayoutProps extends PropsWithChildren {
  scopeId?: string;
  initialFocusId?: string;
  restoreFocus?: boolean;
  rememberScroll?: boolean;
  trapFocus?: boolean;
  className?: string;
}
