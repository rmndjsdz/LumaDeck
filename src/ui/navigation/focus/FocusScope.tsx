import {
  useLayoutEffect,
  useRef,
  type CSSProperties,
  type PropsWithChildren,
} from "react";

import { useNavigation } from "../navigation-context";
import { useNavigationStore } from "../../../stores/navigation-store";
import type { ScopeRegistration } from "../core/navigation-types";
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
  };

  useLayoutEffect(() => {
    return engine.registerScope({
      ...optionsRef.current,
      onBack: () => optionsRef.current.onBack?.(),
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
