import { FocusScope } from "../focus/FocusScope";
import type { NavigationLayoutProps } from "./layout-types";

export interface NavigationDialogProps extends NavigationLayoutProps {
  onBack?: () => boolean | void;
}

export function NavigationDialog({
  scopeId,
  initialFocusId,
  restoreFocus = true,
  rememberScroll = true,
  className,
  onBack,
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
      onBack={onBack}
    >
      {content}
    </FocusScope>
  );
}
