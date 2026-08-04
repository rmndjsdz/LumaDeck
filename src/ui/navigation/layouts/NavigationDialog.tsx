import { FocusScope } from "../focus/FocusScope";
import type { ScopeActionHandler } from "../core/navigation-types";
import type { NavigationLayoutProps } from "./layout-types";

export interface NavigationDialogProps extends NavigationLayoutProps {
  onBack?: () => boolean | void;
  onAction?: ScopeActionHandler;
}

export function NavigationDialog({
  scopeId,
  initialFocusId,
  restoreFocus = true,
  rememberScroll = true,
  className,
  onBack,
  onAction,
  children,
}: NavigationDialogProps) {
  const content = (
    <div
      className={`navigation-dialog ${className ?? ""}`}
      role="dialog"
      aria-modal="true"
      data-navigation-dialog="true"
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
      trapFocus
      modal
      activateOnMount
      onBack={onBack}
      onAction={onAction}
    >
      {content}
    </FocusScope>
  );
}
