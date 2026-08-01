import { DIRECTION_TO_ACTION } from "../core/navigation-actions";
import type {
  NavigationAction,
  NavigationDirection,
} from "../core/navigation-types";
import { DirectionRepeatController } from "./direction-repeat-controller";

export type GamepadStateSource = () => readonly (Gamepad | null)[];

interface GamepadAdapterOptions {
  onAction: (action: NavigationAction) => void;
  onInputMode: () => void;
  onConnectionChange: (connected: boolean) => void;
  repeatController: DirectionRepeatController;
  deadzone?: number;
  source?: GamepadStateSource;
  target?: Window;
  requestFrame?: (callback: FrameRequestCallback) => number;
  cancelFrame?: (handle: number) => void;
}

export class GamepadAdapter {
  private readonly target: Window | null;
  private readonly onAction: (action: NavigationAction) => void;
  private readonly onInputMode: () => void;
  private readonly onConnectionChange: (connected: boolean) => void;
  private readonly repeatController: DirectionRepeatController;
  private readonly deadzone: number;
  private readonly source: GamepadStateSource;
  private readonly requestFrame: (callback: FrameRequestCallback) => number;
  private readonly cancelFrame: (handle: number) => void;
  private readonly previousButtons = new Map<number, boolean>();
  private frameHandle: number | null = null;
  private running = false;
  private connected = false;
  private currentDirection: NavigationDirection | null = null;

  public constructor(options: GamepadAdapterOptions) {
    this.target =
      options.target ?? (typeof window === "undefined" ? null : window);
    this.onAction = options.onAction;
    this.onInputMode = options.onInputMode;
    this.onConnectionChange = options.onConnectionChange;
    this.repeatController = options.repeatController;
    this.deadzone = options.deadzone ?? 0.35;
    this.source =
      options.source ??
      (() =>
        typeof navigator === "undefined" ||
        typeof navigator.getGamepads !== "function"
          ? []
          : navigator.getGamepads());
    this.requestFrame =
      options.requestFrame ??
      ((callback) => {
        if (typeof window === "undefined") return 0;
        return window.setTimeout(() => callback(performance.now()), 16);
      });
    this.cancelFrame =
      options.cancelFrame ??
      ((handle) => {
        if (typeof window !== "undefined") window.clearTimeout(handle);
      });
  }

  public start(): void {
    if (this.running) return;
    this.running = true;
    this.target?.addEventListener("gamepadconnected", this.handleConnection);
    this.target?.addEventListener("gamepaddisconnected", this.handleConnection);
    this.poll();
  }

  public stop(): void {
    this.running = false;
    if (this.frameHandle !== null) this.cancelFrame(this.frameHandle);
    this.frameHandle = null;
    this.repeatController.stop();
    this.currentDirection = null;
    this.previousButtons.clear();
    this.target?.removeEventListener("gamepadconnected", this.handleConnection);
    this.target?.removeEventListener(
      "gamepaddisconnected",
      this.handleConnection,
    );
    if (this.connected) this.onConnectionChange(false);
    this.connected = false;
  }

  public dispose(): void {
    this.stop();
  }

  public poll = (): void => {
    if (!this.running) return;
    this.frameHandle = null;
    const gamepad = this.source().find((candidate) => candidate?.connected);
    const isConnected = Boolean(gamepad);
    if (isConnected !== this.connected) {
      this.connected = isConnected;
      this.onConnectionChange(isConnected);
    }
    if (!gamepad) return;

    this.onInputMode();
    this.handleButtons(gamepad);
    this.handleDirection(gamepad);
    this.frameHandle = this.requestFrame(this.poll);
  };

  private handleButtons(gamepad: Gamepad): void {
    const buttonActions: Readonly<Record<number, NavigationAction>> = {
      0: "confirm",
      1: "back",
      4: "page-previous",
      5: "page-next",
    };
    for (const [indexText, action] of Object.entries(buttonActions)) {
      const index = Number(indexText);
      const pressed = gamepad.buttons[index]?.pressed ?? false;
      const wasPressed = this.previousButtons.get(index) ?? false;
      if (pressed && !wasPressed) this.onAction(action);
      this.previousButtons.set(index, pressed);
    }
  }

  private handleDirection(gamepad: Gamepad): void {
    const direction = this.readDirection(gamepad);
    if (direction === this.currentDirection) return;
    this.currentDirection = direction;
    this.repeatController.stop();
    if (!direction) return;
    this.onAction(DIRECTION_TO_ACTION[direction]);
    this.repeatController.start(direction, () => {
      this.onAction(DIRECTION_TO_ACTION[direction]);
    });
  }

  private readDirection(gamepad: Gamepad): NavigationDirection | null {
    const dpad: Readonly<Record<number, NavigationDirection>> = {
      12: "up",
      13: "down",
      14: "left",
      15: "right",
    };
    for (const [indexText, direction] of Object.entries(dpad)) {
      if (gamepad.buttons[Number(indexText)]?.pressed) return direction;
    }

    const x = gamepad.axes[0] ?? 0;
    const y = gamepad.axes[1] ?? 0;
    if (Math.max(Math.abs(x), Math.abs(y)) < this.deadzone) return null;
    if (Math.abs(x) >= Math.abs(y)) return x < 0 ? "left" : "right";
    return y < 0 ? "up" : "down";
  }

  private handleConnection = (): void => {
    if (this.running && this.frameHandle === null) this.poll();
  };
}
