import { describe, expect, it, vi } from "vitest";
import { createReviewsService } from "./reviews-service";
import type { ReviewsGame } from "./reviews-types";

const game: ReviewsGame = {
  id: "steam-440",
  title: "Team Fortress 2",
  details: { steam: { appId: 440 } },
};

describe("reviews service", () => {
  it("does not call a provider when the identifier is missing", async () => {
    const getSources = vi.fn();
    const service = createReviewsService({ getSources });
    const result = await service.getGameReviewsSummary({
      id: "local-1",
      title: "Unknown game",
    });
    expect(result.status).toBe("identifier-missing");
    expect(getSources).not.toHaveBeenCalled();
  });

  it("passes the AbortSignal through and normalizes the response", async () => {
    const controller = new AbortController();
    const getSources = vi.fn(
      async (_game: ReviewsGame, signal?: AbortSignal) => {
        expect(signal).toBe(controller.signal);
        return { gameId: game.id, title: game.title, steamAppId: 440 };
      },
    );
    const service = createReviewsService({ getSources });
    const result = await service.getGameReviewsSummary(game, controller.signal);
    expect(result.status).toBe("no-data");
    expect(getSources).toHaveBeenCalledOnce();
  });

  it("rejects an in-flight request when TanStack Query aborts it", async () => {
    const controller = new AbortController();
    const getSources = vi.fn(() => new Promise<unknown>(() => undefined));
    const service = createReviewsService({ getSources });
    const request = service.getGameReviewsSummary(game, controller.signal);
    controller.abort();
    await expect(request).rejects.toMatchObject({ name: "AbortError" });
  });
});
