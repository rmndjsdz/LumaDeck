const isDevelopment = import.meta.env.DEV;

export type PerformanceMarkName =
  | "input-received"
  | "logical-focus-updated"
  | "dom-focus-confirmed"
  | "scroll-completed"
  | "main-content-updated"
  | "background-requested"
  | "background-decoded"
  | "crossfade-started"
  | "crossfade-finished"
  | "view-requested"
  | "view-active";

export function markPerformance(name: PerformanceMarkName): void {
  if (!isDevelopment || typeof performance.mark !== "function") return;
  performance.mark(`lumadeck:${name}`);
}

export function measurePerformance(
  name: string,
  start: PerformanceMarkName,
  end: PerformanceMarkName,
): void {
  if (!isDevelopment || typeof performance.measure !== "function") return;
  try {
    performance.measure(
      `lumadeck:${name}`,
      `lumadeck:${start}`,
      `lumadeck:${end}`,
    );
  } catch {
    // A mark may be unavailable after a fast reload; measurements are advisory.
  }
}
