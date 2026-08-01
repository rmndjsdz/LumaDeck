export interface PerformanceCounters {
  appShellRenders: number;
  gameCardRenders: number;
}

const counters: PerformanceCounters = {
  appShellRenders: 0,
  gameCardRenders: 0,
};

export function recordRender(kind: "app-shell" | "game-card"): void {
  if (!import.meta.env.DEV) return;
  if (kind === "app-shell") counters.appShellRenders += 1;
  else counters.gameCardRenders += 1;
}

export function getPerformanceCounters(): PerformanceCounters {
  return { ...counters };
}
