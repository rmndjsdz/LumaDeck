import { describe, expect, it, vi } from "vitest";
import { BackgroundManager } from "./background-manager";

describe("BackgroundManager", () => {
  it("crossfades only after the incoming image is ready", () => {
    vi.useFakeTimers();
    const images: HTMLImageElement[] = [];
    const manager = new BackgroundManager({
      imageFactory: () => {
        const image = document.createElement("img");
        images.push(image);
        return image;
      },
      reducedMotion: () => false,
      durationMs: 200,
    });

    manager.request("a");
    expect(manager.getSnapshot()).toMatchObject({
      currentUrl: null,
      incomingUrl: "a",
      incomingVisible: false,
    });
    images[0].onload?.(new Event("load"));
    expect(manager.getSnapshot()).toMatchObject({
      currentUrl: null,
      incomingUrl: "a",
      incomingVisible: true,
    });
    vi.advanceTimersByTime(200);
    expect(manager.getSnapshot()).toEqual({
      currentUrl: "a",
      incomingUrl: null,
      incomingVisible: false,
    });
    manager.dispose();
    vi.useRealTimers();
  });

  it("ignores obsolete requests and preserves the current background on failure", () => {
    const images: HTMLImageElement[] = [];
    const manager = new BackgroundManager({
      imageFactory: () => {
        const image = document.createElement("img");
        images.push(image);
        return image;
      },
      reducedMotion: () => true,
    });

    manager.request("a");
    images[0].onload?.(new Event("load"));
    expect(manager.getSnapshot().currentUrl).toBe("a");
    manager.request("b");
    manager.request("c");
    images[1].onload?.(new Event("load"));
    expect(manager.getSnapshot().currentUrl).toBe("a");
    images[2].onerror?.(new Event("error"));
    expect(manager.getSnapshot()).toMatchObject({
      currentUrl: "a",
      incomingUrl: null,
    });
    manager.dispose();
  });
});
