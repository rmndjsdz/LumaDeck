import { describe, expect, it } from "vitest";
import { MediaManager, type MediaDescriptor } from "./media-manager";
import type { Game } from "../../features/catalog/game-types";

function createHarness(maxEntries = 8) {
  const images: HTMLImageElement[] = [];
  const events: string[] = [];
  const manager = new MediaManager({
    maxEntries,
    onEvent: (event) => events.push(event.type),
    imageFactory: () => {
      const image = document.createElement("img");
      Object.defineProperty(image, "decode", {
        configurable: true,
        value: () => Promise.resolve(),
      });
      images.push(image);
      return image;
    },
  });
  return { manager, images, events };
}

function descriptor(url: string): MediaDescriptor {
  return { gameId: "game-a", mediaType: "screenshot", url };
}

const homeGame: Game = {
  id: "game-a",
  title: "Game A",
  sortTitle: "Game A",
  platform: "PC",
  provider: "test",
  coverUrl: "a",
  verticalCoverUrl: "",
  logoUrl: "",
  backgroundUrl: "",
  screenshots: [],
  description: "",
  genres: [],
  releaseYear: 2026,
  playtimeMinutes: 0,
  lastPlayedAt: null,
  favorite: false,
  installed: true,
  progress: 0,
  status: "not-started",
};

async function resolveImage(image: HTMLImageElement): Promise<void> {
  image.onload?.(new Event("load"));
  await Promise.resolve();
}

describe("MediaManager", () => {
  it("deduplicates a request and reports a visual cache hit", async () => {
    const { manager, images, events } = createHarness();

    const first = manager.ensure(descriptor("a"));
    const second = manager.ensure(descriptor("a"));
    expect(first).toBe(second);
    expect(images).toHaveLength(1);
    await resolveImage(images[0]);
    await first;
    await manager.ensure(descriptor("a"));

    expect(events).toContain("MEDIA_CACHE_MISS");
    expect(events).toContain("MEDIA_CACHE_INSERT");
    expect(events).toContain("VISUAL_CACHE_HIT");
    manager.dispose();
  });

  it("retains the decoded resource after its route consumer unmounts", async () => {
    const { manager, images } = createHarness();
    const pending = manager.ensure(descriptor("a"));
    await resolveImage(images[0]);
    await pending;

    expect(manager.getSnapshot("a").state).toBe("ready");
    expect(manager.getSnapshot("a").image).toBe(images[0]);
    manager.dispose();
  });

  it("evicts the least recently used non-hot resource", async () => {
    const { manager, images } = createHarness(2);
    const first = manager.ensure(descriptor("a"));
    await resolveImage(images[0]);
    await first;
    const second = manager.ensure(descriptor("b"));
    await resolveImage(images[1]);
    await second;
    manager.getSnapshot("b");
    const third = manager.ensure(descriptor("c"));
    await resolveImage(images[2]);
    await third;

    expect(manager.getSnapshot("a").state).toBe("idle");
    expect(manager.getSnapshot("b").state).toBe("ready");
    expect(manager.getStats().entries).toBe(2);
    manager.dispose();
  });

  it("keeps Home hotset artwork resident while another resource is requested", async () => {
    const { manager, images } = createHarness(1);
    const home = manager.ensure({
      gameId: homeGame.id,
      mediaType: "grid",
      url: "a",
    });
    await resolveImage(images[0]);
    await home;
    manager.setHomeHotset([homeGame]);

    const other = manager.ensure(descriptor("b"));
    await resolveImage(images[1]);
    await other;

    expect(manager.getSnapshot("a").state).toBe("ready");
    manager.dispose();
  });

  it("reuses A immediately after the A to B to A route sequence", async () => {
    const { manager, images, events } = createHarness();
    const visit = async (url: string): Promise<void> => {
      const before = images.length;
      const pending = manager.ensure(descriptor(url));
      if (images.length > before) await resolveImage(images[before]);
      await pending;
    };

    await visit("a");
    await visit("b");
    await visit("a");

    expect(manager.getSnapshot("a").state).toBe("ready");
    expect(events).toContain("VISUAL_CACHE_HIT");
    manager.dispose();
  });
});
