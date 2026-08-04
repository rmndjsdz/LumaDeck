import { describe, expect, it } from "vitest";

import {
  applyCharacter,
  createKeyboardEditState,
  insertSpace,
  removeLastCharacter,
  shouldUseUppercase,
} from "./keyboard-session";

describe("virtual keyboard edit session", () => {
  it("keeps original and draft values separate and supports Ñ and spaces", () => {
    const original = createKeyboardEditState("A");
    const withEnye = applyCharacter({ ...original, capsLock: true }, "ñ");
    const withSpace = insertSpace(withEnye);

    expect(original).toMatchObject({ originalValue: "A", draftValue: "A" });
    expect(withSpace.draftValue).toBe("AÑ ");
    expect(removeLastCharacter(withSpace).draftValue).toBe("AÑ");
  });

  it("models temporary Shift and Caps Lock as physical keyboard XOR", () => {
    expect(shouldUseUppercase(false, false, false)).toBe(false);
    expect(shouldUseUppercase(false, true, false)).toBe(true);
    expect(shouldUseUppercase(true, false, false)).toBe(true);
    expect(shouldUseUppercase(true, true, false)).toBe(false);
    expect(
      applyCharacter(
        {
          ...createKeyboardEditState(""),
          capsLock: true,
          temporaryShift: true,
        },
        "A",
      ).draftValue,
    ).toBe("a");
  });

  it("supports numeric limits and secure drafts without changing the generic session", () => {
    const session = createKeyboardEditState("", {
      inputMode: "numeric",
      maxLength: 3,
      secure: true,
    });
    const numeric = applyCharacter(applyCharacter(session, "1"), "A");
    const limited = applyCharacter(applyCharacter(numeric, "2"), "3");

    expect(numeric.draftValue).toBe("1");
    expect(limited.draftValue).toBe("123");
    expect(limited.secure).toBe(true);
    expect(insertSpace(limited).draftValue).toBe("123");
  });
});
