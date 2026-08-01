import type { NavigationDirection } from "../core/navigation-types";

export interface RepeatControllerOptions {
  initialDelayMs?: number;
  intervalMs?: number;
  acceleratedIntervalMs?: number;
  accelerationAfter?: number;
}

export class DirectionRepeatController {
  private activeDirection: NavigationDirection | null = null;
  private timeoutId: ReturnType<typeof setTimeout> | null = null;
  private repeatCount = 0;
  private callback: (() => void) | null = null;
  private readonly options: Required<RepeatControllerOptions>;

  public constructor(options: RepeatControllerOptions = {}) {
    this.options = {
      initialDelayMs: options.initialDelayMs ?? 260,
      intervalMs: options.intervalMs ?? 90,
      acceleratedIntervalMs: options.acceleratedIntervalMs ?? 58,
      accelerationAfter: options.accelerationAfter ?? 6,
    };
  }

  public start(direction: NavigationDirection, callback: () => void): void {
    this.stop();
    this.activeDirection = direction;
    this.callback = callback;
    this.repeatCount = 0;
    this.schedule(this.options.initialDelayMs);
  }

  public stop(): void {
    if (this.timeoutId !== null) clearTimeout(this.timeoutId);
    this.timeoutId = null;
    this.activeDirection = null;
    this.callback = null;
    this.repeatCount = 0;
  }

  public getActiveDirection(): NavigationDirection | null {
    return this.activeDirection;
  }

  public dispose(): void {
    this.stop();
  }

  private schedule(delayMs: number): void {
    this.timeoutId = setTimeout(() => {
      this.timeoutId = null;
      if (!this.activeDirection || !this.callback) return;
      this.repeatCount += 1;
      this.callback();
      const interval =
        this.repeatCount >= this.options.accelerationAfter
          ? this.options.acceleratedIntervalMs
          : this.options.intervalMs;
      this.schedule(interval);
    }, delayMs);
  }
}
