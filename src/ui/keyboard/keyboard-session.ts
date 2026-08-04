export interface KeyboardEditState {
  originalValue: string;
  draftValue: string;
  inputMode: "text" | "numeric";
  maxLength?: number;
  secure: boolean;
  capsLock: boolean;
  temporaryShift: boolean;
  latchedShift: boolean;
}

export function createKeyboardEditState(
  value: string,
  options: {
    inputMode?: "text" | "numeric";
    maxLength?: number;
    secure?: boolean;
  } = {},
): KeyboardEditState {
  return {
    originalValue: value,
    draftValue: value,
    inputMode: options.inputMode ?? "text",
    maxLength: options.maxLength,
    secure: options.secure ?? false,
    capsLock: false,
    temporaryShift: false,
    latchedShift: false,
  };
}

export function shouldUseUppercase(
  capsLock: boolean,
  temporaryShift: boolean,
  latchedShift: boolean,
): boolean {
  return capsLock !== (temporaryShift || latchedShift);
}

export function applyCharacter(
  state: KeyboardEditState,
  value: string,
): KeyboardEditState {
  if (state.inputMode === "numeric" && !/^\d$/.test(value)) return state;
  if (
    state.maxLength !== undefined &&
    state.draftValue.length >= state.maxLength
  )
    return state;
  const uppercase = shouldUseUppercase(
    state.capsLock,
    state.temporaryShift,
    state.latchedShift,
  );
  const nextValue = uppercase
    ? value.toLocaleUpperCase("es-SV")
    : value.toLocaleLowerCase("es-SV");
  return {
    ...state,
    draftValue: `${state.draftValue}${nextValue}`,
    latchedShift: false,
  };
}

export function removeLastCharacter(
  state: KeyboardEditState,
): KeyboardEditState {
  const characters = Array.from(state.draftValue);
  characters.pop();
  return { ...state, draftValue: characters.join("") };
}

export function insertSpace(state: KeyboardEditState): KeyboardEditState {
  if (state.inputMode === "numeric") return state;
  if (
    state.maxLength !== undefined &&
    state.draftValue.length >= state.maxLength
  )
    return state;
  return { ...state, draftValue: `${state.draftValue} ` };
}
