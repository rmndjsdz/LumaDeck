import { describe, expect, it, vi } from "vitest";

import type { NavigationAction } from "../core/navigation-types";
import { DirectionRepeatController } from "./direction-repeat-controller";
import { GamepadAdapter } from "./gamepad-adapter";

interface GamepadValues {
  leftTrigger?: number;
  rightTrigger?: number;
  axes?: number[];
  rightDpad?: boolean;
}

function makeGamepad(values: GamepadValues = {}): Gamepad {
  const leftTrigger = values.leftTrigger ?? 0;
  const rightTrigger = values.rightTrigger ?? 0;
  const buttons = Array.from({ length: 16 }, (_, index) => ({
    pressed:
      index === 6
        ? leftTrigger >= 0.75
        : index === 7
          ? rightTrigger >= 0.75
          : index === 15 && values.rightDpad === true,
    touched: false,
    value:
      index === 6
        ? leftTrigger
        : index === 7
          ? rightTrigger
          : index === 15 && values.rightDpad === true
            ? 1
            : 0,
  }));
  return {
    axes: values.axes ?? [0, 0, 0, 0, 0, 0],
    buttons,
    connected: true,
    id: "test-pad",
    index: 0,
    mapping: "standard",
    timestamp: 0,
    vibrationActuator: {
      playEffect: async () => "complete",
      reset: async () => "complete",
    },
  };
}

function createHarness(initial: GamepadValues = {}) {
  let values = initial;
  const frameCallbacks: FrameRequestCallback[] = [];
  const actions: NavigationAction[] = [];
  const adapter = new GamepadAdapter({
    source: () => [makeGamepad(values)],
    requestFrame: (callback) => {
      frameCallbacks.push(callback);
      return frameCallbacks.length;
    },
    cancelFrame: vi.fn(),
    repeatController: new DirectionRepeatController(),
    onAction: (action) => actions.push(action),
    onInputMode: vi.fn(),
    onConnectionChange: vi.fn(),
  });
  const tick = (nextValues: GamepadValues) => {
    values = nextValues;
    frameCallbacks[frameCallbacks.length - 1]?.(performance.now());
  };
  adapter.start();
  return { actions, adapter, tick };
}

describe("GamepadAdapter", () => {
  it("handles d-pad transitions, connection state, and cleanup", () => {
    const diagnostics: Array<{
      direction: string | null;
      buttons: number[];
    }> = [];
    const actions: NavigationAction[] = [];
    let rightPressed = false;
    const frameCallbacks: FrameRequestCallback[] = [];
    const adapter = new GamepadAdapter({
      source: () => [makeGamepad({ rightDpad: rightPressed })],
      requestFrame: (callback) => {
        frameCallbacks.push(callback);
        return 1;
      },
      cancelFrame: vi.fn(),
      repeatController: new DirectionRepeatController(),
      onAction: (action) => actions.push(action),
      onInputMode: vi.fn(),
      onConnectionChange: vi.fn(),
      onDiagnostic: (diagnostic) => {
        if (diagnostic) {
          diagnostics.push({
            direction: diagnostic.direction,
            buttons: diagnostic.pressedButtons,
          });
        }
      },
    });

    adapter.start();
    expect(actions).toEqual([]);
    rightPressed = true;
    frameCallbacks[0]?.(performance.now());
    expect(actions).toEqual(["move-right"]);
    expect(diagnostics[diagnostics.length - 1]).toEqual({
      direction: "right",
      buttons: [15],
    });
    adapter.stop();
    expect(frameCallbacks).toHaveLength(2);
  });

  it("keeps polling when the gamepad connects after startup", () => {
    let connected = false;
    const actions: NavigationAction[] = [];
    const frameCallbacks: FrameRequestCallback[] = [];
    const adapter = new GamepadAdapter({
      source: () => (connected ? [makeGamepad({ rightDpad: true })] : []),
      requestFrame: (callback) => {
        frameCallbacks.push(callback);
        return frameCallbacks.length;
      },
      cancelFrame: vi.fn(),
      repeatController: new DirectionRepeatController(),
      onAction: (action) => actions.push(action),
      onInputMode: vi.fn(),
      onConnectionChange: vi.fn(),
    });

    adapter.start();
    expect(frameCallbacks).toHaveLength(1);

    connected = true;
    frameCallbacks[0]?.(performance.now());

    expect(actions).toEqual(["move-right"]);
    adapter.stop();
  });

  it("maps LT crossing the press threshold to previous-primary-screen", () => {
    const harness = createHarness();

    harness.tick({ leftTrigger: 0.8 });

    expect(harness.actions).toEqual(["previous-primary-screen"]);
    harness.adapter.stop();
  });

  it("maps RT crossing the press threshold to next-primary-screen", () => {
    const harness = createHarness();

    harness.tick({ rightTrigger: 0.8 });

    expect(harness.actions).toEqual(["next-primary-screen"]);
    harness.adapter.stop();
  });

  it.each([
    ["leftTrigger", "previous-primary-screen"],
    ["rightTrigger", "next-primary-screen"],
  ] as const)("does not repeat while %s remains held", (trigger, action) => {
    const harness = createHarness();
    harness.tick({ [trigger]: 0.8 });
    harness.tick({ [trigger]: 0.9 });
    harness.tick({ [trigger]: 0.7 });

    expect(harness.actions).toEqual([action]);
    harness.adapter.stop();
  });

  it("requires release below the hysteresis threshold before a second press", () => {
    const harness = createHarness();
    harness.tick({ leftTrigger: 0.8 });
    harness.tick({ leftTrigger: 0.56 });
    harness.tick({ leftTrigger: 0.54 });
    harness.tick({ leftTrigger: 0.8 });

    expect(harness.actions).toEqual([
      "previous-primary-screen",
      "previous-primary-screen",
    ]);
    harness.adapter.stop();
  });

  it("ignores noise around the threshold while keeping the trigger latched", () => {
    const harness = createHarness();
    harness.tick({ leftTrigger: 0.74 });
    harness.tick({ leftTrigger: 0.76 });
    harness.tick({ leftTrigger: 0.71 });
    harness.tick({ leftTrigger: 0.76 });
    harness.tick({ leftTrigger: 0.54 });
    harness.tick({ leftTrigger: 0.76 });

    expect(harness.actions).toEqual([
      "previous-primary-screen",
      "previous-primary-screen",
    ]);
    harness.adapter.stop();
  });

  it("processes LT and RT independently and supports axis-backed triggers", () => {
    const harness = createHarness();
    harness.tick({ leftTrigger: 0.8 });
    harness.tick({ rightTrigger: 0.8 });
    harness.tick({ rightTrigger: 0 });
    harness.tick({ axes: [0, 0, 0, 0, 0, 0.8] });

    expect(harness.actions).toEqual([
      "previous-primary-screen",
      "next-primary-screen",
      "next-primary-screen",
    ]);
    harness.adapter.stop();
  });
});
