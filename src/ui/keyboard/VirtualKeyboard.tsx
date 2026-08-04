import { useEffect, useMemo, useRef, useState } from "react";

import { Focusable } from "../navigation/focus/Focusable";
import { NavigationDialog } from "../navigation/layouts/NavigationDialog";
import { NavigationRow } from "../navigation/layouts/NavigationRow";
import { NavigationRowGroup } from "../navigation/layouts/NavigationRowGroup";
import type {
  InputSource,
  NavigationAction,
} from "../navigation/core/navigation-types";
import {
  getKeyboardLayout,
  type KeyboardKeyDefinition,
} from "./keyboard-layouts";
import type { KeyboardSession } from "./virtual-keyboard-context";
import { shouldUseUppercase } from "./keyboard-session";

interface VirtualKeyboardProps {
  session: KeyboardSession;
  onAction: (action: NavigationAction, inputSource: InputSource) => boolean;
  onCancel: () => void;
  onCommit: () => void;
  onCharacter: (value: string) => void;
  onBackspace: () => void;
  onSpace: () => void;
  onCapsLock: () => void;
  onShift: () => void;
}

export function VirtualKeyboard({
  session,
  onAction,
  onCancel,
  onCommit,
  onCharacter,
  onBackspace,
  onSpace,
  onCapsLock,
  onShift,
}: VirtualKeyboardProps) {
  const layout = getKeyboardLayout(
    session.inputMode === "numeric" ? "numeric" : "es-SV",
  );
  const [symbolsVisible, setSymbolsVisible] = useState(false);
  const [pressedKey, setPressedKey] = useState<string | null>(null);
  const frameRef = useRef<number | null>(null);
  const rows =
    session.inputMode === "numeric"
      ? layout.rows
      : symbolsVisible
        ? layout.symbolRows
        : layout.rows;
  const uppercase = shouldUseUppercase(
    session.capsLock,
    session.temporaryShift,
    session.latchedShift,
  );
  const firstKeyId = rows[0]?.[0]?.id ?? "1";

  useEffect(
    () => () => {
      if (frameRef.current !== null) {
        window.cancelAnimationFrame(frameRef.current);
      }
    },
    [],
  );

  const pulse = (keyId: string) => {
    setPressedKey(keyId);
    if (frameRef.current !== null)
      window.cancelAnimationFrame(frameRef.current);
    frameRef.current = window.requestAnimationFrame(() => {
      frameRef.current = null;
      setPressedKey(null);
    });
  };

  const handleKey = (key: KeyboardKeyDefinition) => {
    pulse(key.id);
    switch (key.kind) {
      case "character":
        onCharacter(key.value ?? "");
        return;
      case "backspace":
        onBackspace();
        return;
      case "space":
        onSpace();
        return;
      case "enter":
        onCommit();
        return;
      case "cancel":
        onCancel();
        return;
      case "caps-lock":
        onCapsLock();
        return;
      case "shift":
        onShift();
        return;
      case "symbols":
        setSymbolsVisible((visible) => !visible);
        return;
    }
  };

  const statusLabel = useMemo(() => {
    if (session.capsLock && session.temporaryShift) return "minúsculas";
    if (uppercase) return "MAYÚSCULAS";
    return "minúsculas";
  }, [session.capsLock, session.temporaryShift, uppercase]);

  return (
    <div className="virtual-keyboard-backdrop" data-keyboard-modal="true">
      <NavigationDialog
        scopeId="virtual-keyboard"
        initialFocusId={`virtual-key-${firstKeyId}`}
        className="virtual-keyboard-dialog"
        onBack={onCancel}
        onAction={onAction}
      >
        <div className="virtual-keyboard-header">
          <div>
            <p className="eyebrow">Teclado virtual · {layout.locale}</p>
            <h2>
              {session.secure ? "Escribe tu credencial" : "Escribe un valor"}
            </h2>
          </div>
          <span
            className={`virtual-keyboard-caps ${uppercase ? "is-on" : ""}`}
            aria-live="polite"
          >
            ⇪ {statusLabel}
          </span>
        </div>
        <output className="virtual-keyboard-draft" aria-live="polite">
          {session.secure
            ? "•".repeat(Array.from(session.draftValue).length) || " "
            : session.draftValue || " "}
        </output>
        <NavigationRowGroup
          scopeId="virtual-keyboard"
          groupId="virtual-keyboard-rows"
          preserveHorizontalIntent
          className="virtual-keyboard-rows"
        >
          {rows.map((row, rowIndex) => (
            <NavigationRow
              key={`${symbolsVisible ? "symbols" : "letters"}-${rowIndex}`}
              rowId={`virtual-keyboard-row-${rowIndex}`}
              rowIndex={rowIndex}
              preserveHorizontalIntent
              className="virtual-keyboard-row"
            >
              {row.map((key) => (
                <Focusable
                  key={key.id}
                  focusId={`virtual-key-${key.id}`}
                  scopeId="virtual-keyboard"
                  className={`virtual-key ${key.wide ? "is-wide" : ""} ${
                    pressedKey === key.id ? "is-pressed" : ""
                  }`}
                  ariaLabel={key.label}
                  onConfirm={() => handleKey(key)}
                >
                  {key.kind === "character"
                    ? uppercase
                      ? key.label.toLocaleUpperCase("es-SV")
                      : key.label
                    : key.label}
                </Focusable>
              ))}
            </NavigationRow>
          ))}
        </NavigationRowGroup>
        <p className="virtual-keyboard-hint">
          A seleccionar · X borrar · Y espacio · LT Shift · R3 Caps · RT aceptar
          · B cancelar
        </p>
      </NavigationDialog>
    </div>
  );
}
