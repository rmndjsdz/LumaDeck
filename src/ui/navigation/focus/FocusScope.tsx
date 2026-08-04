import {
  useLayoutEffect,
  useRef,
  type CSSProperties,
  type PropsWithChildren,
} from "react";

import { useNavigation } from "../navigation-context";
import { useNavigationStore } from "../../../stores/navigation-store";
import type {
  ScopeActionHandler,
  ScopeRegistration,
} from "../core/navigation-types";
import { FocusScopeContext } from "./focus-scope-context";

export interface FocusScopeProps extends PropsWithChildren {
  scopeId: string;
  parentScopeId?: string;
  initialFocusId?: string;
  restoreFocus?: boolean;
  rememberScroll?: boolean;
  trapFocus?: boolean;
  modal?: boolean;
  activateOnMount?: boolean;
  onBack?: () => boolean | void;
  onAction?: ScopeActionHandler;
  className?: string;
  style?: CSSProperties;
}

export function FocusScope({
  scopeId,
  parentScopeId,
  initialFocusId,
  restoreFocus = true,
  rememberScroll = true,
  trapFocus = false,
  modal = false,
  activateOnMount = false,
  onBack,
  onAction,
  className,
  style,
  children,
}: FocusScopeProps) {
  const { engine } = useNavigation();
  const activeScopeId = useNavigationStore((state) => state.activeScopeId);
  const optionsRef = useRef<ScopeRegistration>({
    scopeId,
    parentScopeId,
    initialFocusId,
    restoreFocus,
    rememberScroll,
    trapFocus,
    modal,
    activateOnMount,
    onBack,
    onAction,
  });
  optionsRef.current = {
    scopeId,
    parentScopeId,
    initialFocusId,
    restoreFocus,
    rememberScroll,
    trapFocus,
    modal,
    activateOnMount,
    onBack,
    onAction,
  };

  useLayoutEffect(() => {
    return engine.registerScope({
      ...optionsRef.current,
      onBack: () => optionsRef.current.onBack?.(),
      onAction: (action, inputSource) =>
        optionsRef.current.onAction?.(action, inputSource),
    });
  }, [engine, scopeId]);

  return (
    <FocusScopeContext.Provider value={scopeId}>
      <div
        className={className}
        style={style}
        data-focus-scope={scopeId}
        data-active-scope={activeScopeId === scopeId}
      >
        {children}
      </div>
    </FocusScopeContext.Provider>
  );
}
