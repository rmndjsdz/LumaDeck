import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type PropsWithChildren,
} from "react";

import { useNavigation } from "../navigation/navigation-context";
import type { NavigationAction } from "../navigation/core/navigation-types";
import {
  applyCharacter,
  createKeyboardEditState,
  insertSpace,
  removeLastCharacter,
} from "./keyboard-session";
import { VirtualKeyboard } from "./VirtualKeyboard";
import {
  VirtualKeyboardContext,
  type GamepadTextInputOpenOptions,
  type KeyboardSession,
} from "./virtual-keyboard-context";
import { navigationRuntimeTrace } from "../navigation/debug/navigation-runtime-trace";

const VIRTUAL_KEYBOARD_SCOPE_ID = "virtual-keyboard";

export function VirtualKeyboardProvider({ children }: PropsWithChildren) {
  const { engine } = useNavigation();
  const [session, setSession] = useState<KeyboardSession | null>(null);
  const sessionRef = useRef<KeyboardSession | null>(null);
  const callbacksRef = useRef<{
    onCommit: (value: string) => void;
    onCancel?: (value: string) => void;
  } | null>(null);
  const transitionCounterRef = useRef(0);

  const updateSession = useCallback(
    (update: (current: KeyboardSession) => KeyboardSession) => {
      setSession((current) => {
        if (!current) return current;
        const next = update(current);
        sessionRef.current = next;
        return next;
      });
    },
    [],
  );

  const close = useCallback(
    (reason: "commit" | "cancel") => {
      const current = sessionRef.current;
      const callbacks = callbacksRef.current;
      if (!current || !callbacks) return false;
      navigationRuntimeTrace.setKeyboardState(
        reason === "commit" ? "confirming" : "closing",
      );
      sessionRef.current = null;
      callbacksRef.current = null;
      if (reason === "commit") callbacks.onCommit(current.draftValue);
      else callbacks.onCancel?.(current.originalValue);
      const sourceScopeId = current.sourceScopeId;
      if (sourceScopeId) {
        engine.requestScopeRestore(
          VIRTUAL_KEYBOARD_SCOPE_ID,
          sourceScopeId,
          `virtual-keyboard-${++transitionCounterRef.current}`,
        );
      }
      setSession(null);
      navigationRuntimeTrace.setKeyboardState("closed");
      return true;
    },
    [engine],
  );

  const open = useCallback(
    (options: GamepadTextInputOpenOptions) => {
      if (sessionRef.current) return;
      const sourceScopeId = engine.getActiveScopeId();
      if (!sourceScopeId) return;
      engine.prepareScopeOpen(VIRTUAL_KEYBOARD_SCOPE_ID, options.sourceInputId);
      navigationRuntimeTrace.record("keyboard_open", {
        virtualKeyboardState: "open",
        details: { sourceInputId: options.sourceInputId },
      });
      const next: KeyboardSession = {
        ...createKeyboardEditState(options.value, options),
        sourceInputId: options.sourceInputId,
        sourceScopeId,
      };
      callbacksRef.current = {
        onCommit: options.onCommit,
        onCancel: options.onCancel,
      };
      sessionRef.current = next;
      setSession(next);
    },
    [engine],
  );

  const handleAction = useCallback(
    (action: NavigationAction): boolean => {
      if (!sessionRef.current) return false;
      switch (action) {
        case "previous-primary-screen":
          updateSession((current) => ({ ...current, temporaryShift: true }));
          return true;
        case "shift-release":
          updateSession((current) => ({ ...current, temporaryShift: false }));
          return true;
        case "next-primary-screen":
        case "accept-text":
          return close("commit");
        case "delete-character":
          updateSession((current) => ({
            ...current,
            ...removeLastCharacter(current),
          }));
          return true;
        case "insert-space":
          updateSession((current) => ({ ...current, ...insertSpace(current) }));
          return true;
        case "toggle-caps-lock":
          updateSession((current) => ({
            ...current,
            capsLock: !current.capsLock,
          }));
          return true;
        case "back":
          return close("cancel");
        default:
          return false;
      }
    },
    [close, updateSession],
  );

  useEffect(() => {
    if (!session) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Shift") {
        updateSession((current) => ({ ...current, temporaryShift: true }));
        return;
      }
      if (
        event.key.length !== 1 ||
        event.key.trim().length === 0 ||
        event.ctrlKey ||
        event.metaKey ||
        event.altKey
      ) {
        return;
      }
      event.preventDefault();
      updateSession((current) => ({
        ...current,
        ...applyCharacter(current, event.key),
      }));
    };
    const handleKeyUp = (event: KeyboardEvent) => {
      if (event.key === "Shift") {
        updateSession((current) => ({ ...current, temporaryShift: false }));
      }
    };
    const handleBlur = () => {
      updateSession((current) => ({ ...current, temporaryShift: false }));
    };
    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    window.addEventListener("blur", handleBlur);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
      window.removeEventListener("blur", handleBlur);
    };
  }, [session, updateSession]);

  const contextValue = useMemo(() => ({ session, open }), [open, session]);

  return (
    <VirtualKeyboardContext.Provider value={contextValue}>
      {children}
      {session && (
        <VirtualKeyboard
          session={session}
          onAction={handleAction}
          onCancel={() => close("cancel")}
          onCommit={() => close("commit")}
          onCharacter={(value) =>
            updateSession((current) => ({
              ...current,
              ...applyCharacter(current, value),
            }))
          }
          onBackspace={() =>
            updateSession((current) => ({
              ...current,
              ...removeLastCharacter(current),
            }))
          }
          onSpace={() =>
            updateSession((current) => ({
              ...current,
              ...insertSpace(current),
            }))
          }
          onCapsLock={() =>
            updateSession((current) => ({
              ...current,
              capsLock: !current.capsLock,
            }))
          }
          onShift={() =>
            updateSession((current) => ({
              ...current,
              latchedShift: !current.latchedShift,
            }))
          }
        />
      )}
    </VirtualKeyboardContext.Provider>
  );
}
