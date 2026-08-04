import { useCallback, type ChangeEvent, type KeyboardEvent } from "react";

import {
  useFocusable,
  type UseFocusableOptions,
} from "../navigation/focus/useFocusable";
import { useVirtualKeyboard } from "./virtual-keyboard-context";

export interface GamepadTextInputProps extends Omit<
  UseFocusableOptions,
  "onConfirm"
> {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  className?: string;
  ariaLabel?: string;
  inputMode?: "text" | "numeric";
  maxLength?: number;
  secure?: boolean;
}

export function GamepadTextInput({
  value,
  onChange,
  placeholder,
  className,
  ariaLabel,
  inputMode = "text",
  maxLength,
  secure = false,
  ...focusOptions
}: GamepadTextInputProps) {
  const { open } = useVirtualKeyboard();
  const openKeyboard = useCallback(() => {
    open({
      sourceInputId: focusOptions.focusId,
      value,
      inputMode,
      maxLength,
      secure,
      onCommit: onChange,
    });
  }, [
    focusOptions.focusId,
    inputMode,
    maxLength,
    onChange,
    open,
    secure,
    value,
  ]);
  const focusable = useFocusable<HTMLInputElement>({
    ...focusOptions,
    onConfirm: openKeyboard,
  });

  const handleChange = (event: ChangeEvent<HTMLInputElement>) => {
    const nextValue =
      inputMode === "numeric"
        ? event.target.value.replace(/\D/g, "")
        : event.target.value;
    onChange(maxLength ? nextValue.slice(0, maxLength) : nextValue);
  };
  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key !== "Enter") return;
    event.preventDefault();
    openKeyboard();
  };

  return (
    <input
      ref={focusable.ref}
      className={className}
      value={value}
      placeholder={placeholder}
      aria-label={ariaLabel}
      type={secure ? "password" : "text"}
      inputMode={inputMode === "numeric" ? "numeric" : "text"}
      maxLength={maxLength}
      data-focusable="true"
      data-gamepad-text-input="true"
      data-focus-id={focusOptions.focusId}
      data-active={focusable.isActive ? "true" : "false"}
      tabIndex={focusable.tabIndex}
      autoComplete="off"
      onMouseEnter={focusable.onMouseEnter}
      onClick={focusable.onClick}
      onChange={handleChange}
      onKeyDown={handleKeyDown}
    />
  );
}
