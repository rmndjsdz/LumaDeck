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

  it("uses the fallback artwork when the primary background fails", () => {
    const images: HTMLImageElement[] = [];
    const manager = new BackgroundManager({
      imageFactory: () => {
        const image = document.createElement("img");
        images.push(image);
        return image;
      },
      reducedMotion: () => true,
    });

    manager.request("broken-background", "idle", "cover-fallback");
    images[0].onerror?.(new Event("error"));
    expect(images).toHaveLength(2);
    expect(manager.getSnapshot()).toMatchObject({
      currentUrl: null,
      incomingUrl: "cover-fallback",
    });

    images[1].onload?.(new Event("load"));
    expect(manager.getSnapshot().currentUrl).toBe("cover-fallback");
    manager.dispose();
  });

  it("defers rapid navigation and crossfades only the final destination", () => {
    const images: HTMLImageElement[] = [];
    const manager = new BackgroundManager({
      imageFactory: () => {
        const image = document.createElement("img");
        images.push(image);
        return image;
      },
      reducedMotion: () => true,
    });

    manager.request("a", "navigating");
    manager.request("b", "fast-navigating");
    manager.request("c", "fast-navigating");
    expect(images).toHaveLength(0);
    manager.request("c", "settling");
    expect(images).toHaveLength(1);
    images[0].onload?.(new Event("load"));
    expect(manager.getSnapshot().currentUrl).toBe("c");
    manager.dispose();
  });

  it("reuses a ready resource instead of duplicating a request", () => {
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
    manager.request("b");
    images[1].onload?.(new Event("load"));
    manager.request("a");
    expect(images).toHaveLength(2);
    manager.dispose();
  });
});
