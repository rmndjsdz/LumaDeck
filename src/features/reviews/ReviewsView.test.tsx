import { act } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRoot, type Root } from "react-dom/client";
import { describe, expect, it, vi } from "vitest";
import type { GameReviewsSummary, ReviewsGame } from "./reviews-types";
import { ReviewsView } from "./ReviewsView";
import { NavigationProvider } from "../../ui/navigation/NavigationProvider";

const reviewsMocks = vi.hoisted(() => ({
  getGameReviewsSummary: vi.fn(),
}));

vi.mock("./reviews-service", () => ({
  reviewsService: {
    getGameReviewsSummary: reviewsMocks.getGameReviewsSummary,
  },
}));

const game: ReviewsGame = {
  id: "game-001",
  title: "Game 001",
  details: { steam: { appId: 440 } },
};

const summary: GameReviewsSummary = {
  gameId: game.id,
  title: game.title,
  steamAppId: 440,
  status: "partial",
  sources: {
    metacritic: {
      provider: "metacritic",
      status: "unavailable",
      score: null,
      maxScore: 100,
      count: null,
      url: null,
      distribution: null,
      error: null,
    },
    opencritic: {
      provider: "opencritic",
      status: "available",
      score: 84,
      maxScore: 100,
      count: 42,
      url: "https://opencritic.com/game/1/example",
      distribution: null,
      error: null,
    },
    steam: {
      provider: "steam",
      status: "available",
      score: 82,
      maxScore: 100,
      count: 120,
      url: null,
      distribution: null,
      error: null,
    },
  },
  steamRecent: {
    positiveCount: 82,
    neutralCount: 0,
    negativeCount: 18,
    totalCount: 100,
    positivePercent: 82,
    neutralPercent: 0,
    negativePercent: 18,
  },
  steamHistorical: null,
  featuredReviews: [
    {
      id: "review-001",
      provider: "steam",
      author: "Player",
      text: "A concise featured review.",
      recommended: true,
      playtimeHours: 12,
      createdAt: "2026-01-01T00:00:00.000Z",
      language: "english",
      helpfulVotes: 3,
    },
    {
      id: "review-002",
      provider: "steam",
      author: "Another player",
      text: "A concise negative review.",
      recommended: false,
      playtimeHours: 4,
      createdAt: "2026-01-02T00:00:00.000Z",
      language: "spanish",
      helpfulVotes: null,
    },
  ],
  inputFingerprint: null,
  errors: [],
  fetchedAt: "2026-01-01T00:00:00.000Z",
};

function renderReviews(): { host: HTMLDivElement; root: Root } {
  const host = document.createElement("div");
  document.body.appendChild(host);
  const root = createRoot(host);
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  act(() => {
    root.render(
      <NavigationProvider>
        <QueryClientProvider client={queryClient}>
          <ReviewsView game={game} />
        </QueryClientProvider>
      </NavigationProvider>,
    );
  });
  return { host, root };
}

async function flushEffects(): Promise<void> {
  await act(async () => {
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
  });
}

function cleanup(rendered: { host: HTMLDivElement; root: Root }): void {
  act(() => rendered.root.unmount());
  rendered.host.remove();
}

describe("ReviewsView", () => {
  it("keeps the four-column structure for partial provider results", async () => {
    reviewsMocks.getGameReviewsSummary.mockResolvedValue(summary);
    const rendered = renderReviews();

    await flushEffects();

    expect(rendered.host.querySelectorAll(".reviews-column")).toHaveLength(4);
    expect(rendered.host.textContent).toContain("Sin datos disponibles");
    expect(rendered.host.textContent).toContain("A concise featured review.");
    expect(rendered.host.textContent).toContain("82%");
    expect(rendered.host.textContent).not.toContain("0 / 100");
    expect(
      rendered.host.querySelector(".steam-review-thumb.is-positive"),
    ).not.toBeNull();
    expect(
      rendered.host.querySelector(".steam-review-thumb.is-negative"),
    ).not.toBeNull();
    cleanup(rendered);
  });

  it("uses skeleton panels while the query is pending", () => {
    reviewsMocks.getGameReviewsSummary.mockReturnValue(new Promise(() => {}));
    const rendered = renderReviews();

    expect(rendered.host.querySelector(".reviews-loading")).not.toBeNull();
    expect(rendered.host.querySelectorAll(".reviews-skeleton")).toHaveLength(3);
    cleanup(rendered);
  });
});
