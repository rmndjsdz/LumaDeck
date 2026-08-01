import { FocusScope } from "../focus/FocusScope";
import type { NavigationLayoutProps } from "./layout-types";

export function NavigationRow({
  scopeId,
  initialFocusId,
  restoreFocus,
  rememberScroll,
  className,
  children,
}: NavigationLayoutProps) {
  const content = (
    <div
      className={`navigation-row ${className ?? ""}`}
      data-navigation-group="row"
    >
      {children}
    </div>
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
