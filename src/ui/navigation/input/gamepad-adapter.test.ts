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
    const diagnostics: Array<{ direction: string | null; buttons: number[] }> =
      [];
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
});
