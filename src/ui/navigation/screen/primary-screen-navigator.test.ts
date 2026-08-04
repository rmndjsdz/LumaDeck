import { describe, expect, it } from "vitest";

import { PrimaryScreenNavigator } from "./primary-screen-navigator";

type Screen = "home" | "library" | "universes";

function createNavigator(
  currentScreen: string | null = "home",
  screens: readonly { id: Screen; enabled?: boolean }[] = [
    { id: "home" },
    { id: "library" },
  ],
) {
  const transitions: Screen[] = [];
  const navigator = new PrimaryScreenNavigator<Screen>({
    screens,
    getCurrentScreen: () => currentScreen,
    onTransitionRequest: (target) => transitions.push(target),
  });
  return { navigator, transitions };
}

describe("PrimaryScreenNavigator", () => {
  it("resolves next from Home to Library", () => {
    const { navigator, transitions } = createNavigator();

    expect(navigator.handle("next-primary-screen", "gamepad")).toBe(true);
    expect(transitions).toEqual(["library"]);
  });

  it("ignores previous at the first screen and next at the last screen", () => {
    const home = createNavigator("home");
    const library = createNavigator("library");

    expect(home.navigator.handle("previous-primary-screen", "gamepad")).toBe(
      false,
    );
    expect(library.navigator.handle("next-primary-screen", "gamepad")).toBe(
      false,
    );
    expect(home.transitions).toEqual([]);
    expect(library.transitions).toEqual([]);
  });

  it("uses the declarative order and skips disabled destinations safely", () => {
    const { navigator, transitions } = createNavigator("home", [
      { id: "home" },
      { id: "library", enabled: false },
      { id: "universes" },
    ]);

    expect(navigator.handle("next-primary-screen", "gamepad")).toBe(false);
    expect(transitions).toEqual([]);
    expect(navigator.getTrace()).toContainEqual({
      event: "PRIMARY_SCREEN_INPUT_IGNORED",
      reason: "disabled",
      source: "gamepad",
      trigger: "right",
    });
  });

  it.each([
    ["modal", "modal"],
    ["transition-pending", "transition-pending"],
    ["restoration-pending", "restoration-pending"],
  ] as const)("does not transition when the engine reports %s", (_, reason) => {
    const { transitions } = createNavigator();
    const navigator = new PrimaryScreenNavigator<Screen>({
      screens: [{ id: "home" }, { id: "library" }],
      getCurrentScreen: () => "home",
      getBlockReason: () => reason,
      onTransitionRequest: (target) => transitions.push(target),
    });

    expect(navigator.handle("next-primary-screen", "gamepad")).toBe(false);
    expect(transitions).toEqual([]);
    expect(navigator.getTrace()).toContainEqual({
      event: "PRIMARY_SCREEN_INPUT_IGNORED",
      reason,
      source: "gamepad",
      trigger: "right",
    });
  });

  it("falls back safely for an unknown current screen", () => {
    const { navigator, transitions } = createNavigator("details");

    expect(navigator.handle("next-primary-screen", "gamepad")).toBe(false);
    expect(transitions).toEqual([]);
  });
});
