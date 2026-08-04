export type KeyboardKeyKind =
  | "character"
  | "backspace"
  | "shift"
  | "caps-lock"
  | "space"
  | "enter"
  | "symbols"
  | "cancel";

export interface KeyboardKeyDefinition {
  id: string;
  label: string;
  kind: KeyboardKeyKind;
  value?: string;
  wide?: boolean;
}

export interface KeyboardLayoutDefinition {
  locale: string;
  rows: readonly KeyboardKeyDefinition[][];
  symbolRows: readonly KeyboardKeyDefinition[][];
}

const character = (value: string): KeyboardKeyDefinition => ({
  id: value,
  label: value,
  kind: "character",
  value,
});

const utility = (
  id: string,
  label: string,
  kind: Exclude<KeyboardKeyKind, "character">,
  wide = false,
): KeyboardKeyDefinition => ({ id, label, kind, wide });

const row = (...keys: KeyboardKeyDefinition[]): KeyboardKeyDefinition[] => keys;

export const KEYBOARD_LAYOUTS: Readonly<
  Record<string, KeyboardLayoutDefinition>
> = {
  numeric: {
    locale: "numeric",
    rows: [
      row(..."123".split("").map(character)),
      row(..."456".split("").map(character)),
      row(..."789".split("").map(character)),
      row(character("0"), utility("backspace", "⌫", "backspace")),
      row(
        utility("enter", "Listo", "enter"),
        utility("cancel", "Cancelar", "cancel"),
      ),
    ],
    symbolRows: [],
  },
  "es-SV": {
    locale: "es-SV",
    rows: [
      row(..."1234567890".split("").map(character)),
      row(..."qwertyuiop".split("").map(character)),
      row(..."asdfghjklñ".split("").map(character)),
      row(..."zxcvbnm".split("").map(character)),
      row(
        utility("shift", "Shift", "shift"),
        utility("caps-lock", "⇪ Caps", "caps-lock"),
        utility("backspace", "⌫", "backspace"),
        utility("symbols", "Símbolos", "symbols"),
      ),
      row(
        utility("space", "Espacio", "space", true),
        utility("enter", "Enter", "enter"),
        utility("cancel", "Cancelar", "cancel"),
      ),
    ],
    symbolRows: [
      row(..."!@#$%^&*()".split("").map(character)),
      row(..."-_=+[]{}".split("").map(character)),
      row(...`;:'",.<>/?`.split("").map(character)),
      row(
        utility("shift", "Shift", "shift"),
        utility("caps-lock", "⇪ Caps", "caps-lock"),
        utility("backspace", "⌫", "backspace"),
        utility("symbols", "Letras", "symbols"),
      ),
      row(
        utility("space", "Espacio", "space", true),
        utility("enter", "Enter", "enter"),
        utility("cancel", "Cancelar", "cancel"),
      ),
    ],
  },
};

export function getKeyboardLayout(locale = "es-SV"): KeyboardLayoutDefinition {
  return KEYBOARD_LAYOUTS[locale] ?? KEYBOARD_LAYOUTS["es-SV"];
}
