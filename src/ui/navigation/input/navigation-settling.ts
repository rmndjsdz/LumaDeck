import type { NavigationPhase } from "../core/navigation-types";

export interface NavigationSettlingOptions {
  settleAfterMs?: number;
  idleAfterMs?: number;
  fastThresholdMs?: number;
  onPhaseChange: (phase: NavigationPhase) => void;
}

/** Owns the one timer used to classify a directional input burst. */
export class NavigationSettlingController {
  private readonly settleAfterMs: number;
  private readonly idleAfterMs: number;
  private readonly fastThresholdMs: number;
  private readonly onPhaseChange: (phase: NavigationPhase) => void;
  private phase: NavigationPhase = "idle";
  private lastInputAt = -Infinity;
  private timer: number | null = null;

  public constructor(options: NavigationSettlingOptions) {
    this.settleAfterMs = options.settleAfterMs ?? 112;
    this.idleAfterMs = options.idleAfterMs ?? 128;
    this.fastThresholdMs = options.fastThresholdMs ?? 176;
    this.onPhaseChange = options.onPhaseChange;
  }

  public getPhase(): NavigationPhase {
    return this.phase;
  }

  public notifyNavigation(now = performance.now()): void {
    const isRepeat = now - this.lastInputAt < this.fastThresholdMs;
    this.lastInputAt = now;
    this.setPhase(isRepeat ? "fast-navigating" : "navigating");
    this.scheduleSettling();
  }

  public dispose(): void {
    this.clearTimer();
  }

  private scheduleSettling(): void {
    this.clearTimer();
    this.timer = window.setTimeout(() => {
      this.timer = null;
      this.setPhase("settling");
      this.timer = window.setTimeout(() => {
        this.timer = null;
        this.setPhase("idle");
      }, this.idleAfterMs);
    }, this.settleAfterMs);
  }

  private setPhase(phase: NavigationPhase): void {
    if (this.phase === phase) return;
    this.phase = phase;
    this.onPhaseChange(phase);
  }

  private clearTimer(): void {
    if (this.timer !== null) {
      window.clearTimeout(this.timer);
      this.timer = null;
    }
  }
}
