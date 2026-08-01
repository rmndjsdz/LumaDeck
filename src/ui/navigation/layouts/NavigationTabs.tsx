import { FocusScope } from "../focus/FocusScope";
import type { NavigationLayoutProps } from "./layout-types";

export function NavigationTabs({
  scopeId,
  initialFocusId,
  restoreFocus,
  rememberScroll,
  className,
  children,
}: NavigationLayoutProps) {
  const content = (
    <nav className={`navigation-tabs ${className ?? ""}`} aria-label="Sections">
      {children}
    </nav>
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
