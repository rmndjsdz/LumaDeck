export const SCREENSHOT_ZOOM_LEVELS = [100, 150, 200, 250, 300] as const;

export type ScreenshotZoom = (typeof SCREENSHOT_ZOOM_LEVELS)[number];

export interface ScreenshotPan {
  x: number;
  y: number;
}

export const SCREENSHOT_DEFAULT_ZOOM: ScreenshotZoom = 100;
export const SCREENSHOT_DEFAULT_PAN: ScreenshotPan = { x: 0, y: 0 };

export function getCircularScreenshotIndex(
  currentIndex: number,
  total: number,
  delta: -1 | 1,
): number {
  if (total <= 0) return 0;
  return (currentIndex + delta + total) % total;
}

export function getZoomAfterStep(
  currentZoom: ScreenshotZoom,
  delta: -1 | 1,
): ScreenshotZoom {
  const currentIndex = SCREENSHOT_ZOOM_LEVELS.indexOf(currentZoom);
  const nextIndex = Math.min(
    SCREENSHOT_ZOOM_LEVELS.length - 1,
    Math.max(0, currentIndex + delta),
  );
  return SCREENSHOT_ZOOM_LEVELS[nextIndex] ?? SCREENSHOT_DEFAULT_ZOOM;
}

export function clampScreenshotPan(
  pan: ScreenshotPan,
  bounds: ScreenshotPan,
): ScreenshotPan {
  return {
    x: Math.min(bounds.x, Math.max(-bounds.x, pan.x)),
    y: Math.min(bounds.y, Math.max(-bounds.y, pan.y)),
  };
}
