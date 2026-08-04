import { createContext, useContext } from "react";

import type { KeyboardEditState } from "./keyboard-session";

export interface GamepadTextInputOpenOptions {
  sourceInputId: string;
  value: string;
  inputMode?: "text" | "numeric";
  maxLength?: number;
  secure?: boolean;
  onCommit: (value: string) => void;
  onCancel?: (value: string) => void;
}

export interface KeyboardSession extends KeyboardEditState {
  sourceInputId: string;
  sourceScopeId: string;
}

export interface VirtualKeyboardContextValue {
  session: KeyboardSession | null;
  open: (options: GamepadTextInputOpenOptions) => void;
}

export const VirtualKeyboardContext =
  createContext<VirtualKeyboardContextValue | null>(null);

export function useVirtualKeyboard(): VirtualKeyboardContextValue {
  const context = useContext(VirtualKeyboardContext);
  if (!context) {
    throw new Error(
      "useVirtualKeyboard must be used inside VirtualKeyboardProvider",
    );
  }
  return context;
}
