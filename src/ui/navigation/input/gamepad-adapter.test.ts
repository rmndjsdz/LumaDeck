import { describe, expect, it, vi } from "vitest";

import { DirectionRepeatController } from "./direction-repeat-controller";
import { GamepadAdapter } from "./gamepad-adapter";

function makeGamepad(rightPressed: boolean): Gamepad {
  const buttons = Array.from({ length: 16 }, (_, index) => ({
    pressed: index === 15 && rightPressed,
    touched: false,
    value: index === 15 && rightPressed ? 1 : 0,
  }));
  return {
    axes: [0, 0],
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

describe("GamepadAdapter", () => {
  it("handles d-pad transitions, connection state, and cleanup", () => {
    let rightPressed = false;
    const frameCallbacks: FrameRequestCallback[] = [];
    const actions: string[] = [];
    const adapter = new GamepadAdapter({
      source: () => [makeGamepad(rightPressed)],
      requestFrame: (callback) => {
        frameCallbacks.push(callback);
        return 1;
      },
      cancelFrame: vi.fn(),
      repeatController: new DirectionRepeatController(),
      onAction: (action) => actions.push(action),
      onInputMode: vi.fn(),
      onConnectionChange: vi.fn(),
    });

    adapter.start();
    expect(actions).toEqual([]);
    rightPressed = true;
    frameCallbacks[0]?.(performance.now());
    expect(actions).toEqual(["move-right"]);
    adapter.stop();
    expect(frameCallbacks).toHaveLength(2);
  });
});
