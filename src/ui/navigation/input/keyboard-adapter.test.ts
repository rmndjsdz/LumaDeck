import { describe, expect, it, vi } from "vitest";

import { DirectionRepeatController } from "./direction-repeat-controller";
import {
  isEditableTarget,
  KEYBOARD_ACTION_MAP,
  KeyboardAdapter,
} from "./keyboard-adapter";

describe("KeyboardAdapter", () => {
  it("maps keyboard input to abstract actions", () => {
    expect(KEYBOARD_ACTION_MAP.ArrowUp).toBe("move-up");
    expect(KEYBOARD_ACTION_MAP.w).toBe("move-up");
    expect(KEYBOARD_ACTION_MAP.Enter).toBe("confirm");
    expect(KEYBOARD_ACTION_MAP.Escape).toBe("back");
  });

  it("ignores editable targets", () => {
    const input = document.createElement("input");
    document.body.appendChild(input);
    expect(isEditableTarget(input)).toBe(true);
    expect(isEditableTarget(document.body)).toBe(false);
  });

  it("dispatches once and uses the shared repeat controller", () => {
    vi.useFakeTimers();
    const actions: string[] = [];
    const adapter = new KeyboardAdapter({
      target: window,
      repeatController: new DirectionRepeatController({
        initialDelayMs: 10,
        intervalMs: 10,
      }),
      onAction: (action) => actions.push(action),
      onInputMode: vi.fn(),
    });
    const event = new KeyboardEvent("keydown", {
      key: "ArrowRight",
      cancelable: true,
      bubbles: true,
    });

    adapter.handleKeyDown(event);
    expect(actions).toEqual(["move-right"]);
    vi.advanceTimersByTime(10);
    expect(actions).toEqual(["move-right", "move-right"]);
    adapter.handleKeyUp(new KeyboardEvent("keyup", { key: "ArrowRight" }));
    vi.advanceTimersByTime(50);
    expect(actions).toHaveLength(2);
    adapter.dispose();
    vi.useRealTimers();
  });

  it("traps Tab only when the navigation engine consumes it", () => {
    const onTab = vi.fn(() => true);
    const adapter = new KeyboardAdapter({
      target: window,
      repeatController: new DirectionRepeatController(),
      onAction: vi.fn(),
      onTab,
      onInputMode: vi.fn(),
    });
    const event = new KeyboardEvent("keydown", {
      key: "Tab",
      shiftKey: true,
      cancelable: true,
    });

    adapter.handleKeyDown(event);

    expect(onTab).toHaveBeenCalledWith(true);
    expect(event.defaultPrevented).toBe(true);
    adapter.dispose();
  });
});
