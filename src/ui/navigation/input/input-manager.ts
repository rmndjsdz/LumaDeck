import type { NavigationEngine } from "../core/navigation-engine";
import type { InputMode, NavigationAction } from "../core/navigation-types";
import { useNavigationStore } from "../../../stores/navigation-store";
import { DirectionRepeatController } from "./direction-repeat-controller";
import { GamepadAdapter } from "./gamepad-adapter";
import { KeyboardAdapter } from "./keyboard-adapter";
import { MouseAdapter } from "./mouse-adapter";
import { NavigationSettlingController } from "./navigation-settling";
import { markPerformance } from "../../performance/performance-marks";

export class InputManager {
  private readonly repeatController = new DirectionRepeatController();
  private readonly keyboardAdapter: KeyboardAdapter;
  private readonly mouseAdapter: MouseAdapter;
  private readonly gamepadAdapter: GamepadAdapter;
  private started = false;
  private lastDiscreteAction: { action: NavigationAction; at: number } | null =
    null;
  private readonly settlingController: NavigationSettlingController;

  public constructor(private readonly engine: NavigationEngine) {
    this.keyboardAdapter = new KeyboardAdapter({
      repeatController: this.repeatController,
      onAction: (action) => this.dispatch(action, "keyboard"),
      onTab: (shiftKey) => this.engine.handleTab(shiftKey),
      onInputMode: () => this.setInputMode("keyboard"),
    });
    this.mouseAdapter = new MouseAdapter({
      onInputMode: () => this.setInputMode("mouse"),
    });
    this.gamepadAdapter = new GamepadAdapter({
      repeatController: this.repeatController,
      onAction: (action) => this.dispatch(action, "gamepad"),
      onInputMode: () => this.setInputMode("gamepad"),
      onConnectionChange: (connected) =>
        useNavigationStore.getState().setGamepadConnected(connected),
      onDiagnostic: (diagnostic) =>
        useNavigationStore.getState().setGamepadDiagnostic(diagnostic),
    });
    this.settlingController = new NavigationSettlingController({
      onPhaseChange: (phase) => {
        useNavigationStore.getState().setNavigationPhase(phase);
      },
    });
  }

  public start(): void {
    if (this.started) return;
    this.started = true;
    this.keyboardAdapter.start();
    this.mouseAdapter.start();
    this.gamepadAdapter.start();
  }

  public stop(): void {
    if (!this.started) return;
    this.started = false;
    this.keyboardAdapter.stop();
    this.mouseAdapter.stop();
    this.gamepadAdapter.stop();
  }

  public dispose(): void {
    this.stop();
    this.repeatController.dispose();
    this.settlingController.dispose();
  }

  public handlePointerHover(focusId: string): void {
    this.mouseAdapter.markHover();
    this.engine.focus(focusId);
  }

  public handlePointerConfirm(focusId: string): void {
    this.mouseAdapter.markClick();
    if (!this.engine.focus(focusId)) return;
    this.dispatch("confirm", "mouse");
  }

  public setInputMode(inputMode: InputMode): void {
    useNavigationStore.getState().setInputMode(inputMode);
  }

  public dispatch(action: NavigationAction, inputMode: InputMode): boolean {
    this.setInputMode(inputMode);
    markPerformance("input-received");
    if (action.startsWith("move-")) {
      this.settlingController.notifyNavigation();
    }
    const now = performance.now();
    if (
      (action === "confirm" || action === "back") &&
      this.lastDiscreteAction?.action === action &&
      now - this.lastDiscreteAction.at < 50
    ) {
      return false;
    }
    this.lastDiscreteAction = { action, at: now };
    return this.engine.dispatch(action);
  }
}
