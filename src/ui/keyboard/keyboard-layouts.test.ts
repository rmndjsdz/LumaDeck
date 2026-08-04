import { describe, expect, it } from "vitest";

import { getKeyboardLayout } from "./keyboard-layouts";

describe("keyboard layouts", () => {
  it("defines the Spanish El Salvador layout as data", () => {
    const layout = getKeyboardLayout("es-SV");
    const keys = layout.rows.flat().map((key) => key.label);
    expect(layout.locale).toBe("es-SV");
    expect(keys).toEqual(
      expect.arrayContaining([
        "1",
        "0",
        "ñ",
        "Shift",
        "⇪ Caps",
        "⌫",
        "Espacio",
        "Enter",
        "Símbolos",
        "Cancelar",
      ]),
    );
  });

  it("falls back to the default locale without duplicating a keyboard", () => {
    expect(getKeyboardLayout("en-US")).toBe(getKeyboardLayout("es-SV"));
  });
});
