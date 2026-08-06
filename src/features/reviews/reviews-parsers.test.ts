import { describe, expect, it } from "vitest";
import {
  createIdentifierMissingSummary,
  parseGameReviewsSummary,
  parseMetacritic,
  parseOpenCritic,
} from "./reviews-parsers";
import type { ReviewsGame } from "./reviews-types";

const game: ReviewsGame = {
  id: "steam-440",
  title: "Team Fortress 2",
  details: { steam: { appId: 440 } },
};

const completeSources = {
  gameId: game.id,
  title: game.title,
  steamAppId: 440,
  metacritic: {
    score: 92,
    url: "https://www.metacritic.com/game/team-fortress-2/",
  },
  opencritic: {
    id: 440,
    name: game.title,
    score: 88.5,
    reviewCount: 24,
    url: "https://opencritic.com/game/440",
  },
  steam: {
    all: {
      querySummary: {
        totalReviews: 100,
        totalPositive: 80,
        totalNegative: 20,
      },
      reviews: [],
    },
    recent: {
      querySummary: {
        totalReviews: 10,
        totalPositive: 7,
        totalNegative: 3,
      },
      reviews: [
        {
          recommendationId: "review-1",
          author: "Comunidad Steam",
          review: "Sigue siendo divertido.",
          votedUp: true,
          playtimeForeverMinutes: 360,
          timestampCreated: 1_700_000_000,
          language: "spanish",
          votesUp: 12,
        },
      ],
    },
  },
  errors: [],
};

describe("reviews parsers", () => {
  it("normalizes provider scores and URLs", () => {
    expect(parseMetacritic(completeSources.metacritic)).toMatchObject({
      provider: "metacritic",
      status: "available",
      score: 92,
      maxScore: 100,
    });
    expect(parseOpenCritic(completeSources.opencritic)).toMatchObject({
      provider: "opencritic",
      score: 88.5,
      count: 24,
    });
  });

  it("normalizes Steam historical/recent distribution and featured reviews", () => {
    const summary = parseGameReviewsSummary(completeSources, game);
    expect(summary.status).toBe("success");
    expect(summary.steamHistorical).toMatchObject({
      positiveCount: 80,
      negativeCount: 20,
      totalCount: 100,
      positivePercent: 80,
    });
    expect(summary.steamRecent?.positivePercent).toBe(70);
    expect(summary.featuredReviews[0]).toMatchObject({
      id: "review-1",
      playtimeHours: 6,
      language: "spanish",
      helpfulVotes: 12,
    });
  });

  it("uses the historical helpful reviews and keeps only English and Spanish", () => {
    const summary = parseGameReviewsSummary(
      {
        ...completeSources,
        steam: {
          ...completeSources.steam,
          all: {
            ...completeSources.steam.all,
            reviews: [
              {
                recommendationId: "french",
                review: "Avis en français",
                language: "french",
                votedUp: true,
                votesUp: 999,
              },
              {
                recommendationId: "english",
                review: "Useful review",
                language: "english",
                votedUp: true,
                votesUp: 20,
              },
              {
                recommendationId: "spanish",
                review: "Reseña útil",
                language: "spanish",
                votedUp: false,
                votesUp: 10,
              },
            ],
          },
        },
      },
      game,
    );
    expect(summary.featuredReviews.map((review) => review.id)).toEqual([
      "english",
      "spanish",
    ]);
  });

  it("preserves partial data when one provider is down", () => {
    const summary = parseGameReviewsSummary(
      {
        ...completeSources,
        opencritic: null,
        errors: [
          {
            provider: "opencritic",
            code: "network",
            message: "Provider is unreachable",
          },
        ],
      },
      game,
    );
    expect(summary.status).toBe("partial");
    expect(summary.sources.opencritic.status).toBe("error");
    expect(summary.sources.steam.status).toBe("available");
  });

  it("keeps a successful Steam window when its other window fails", () => {
    const summary = parseGameReviewsSummary(
      {
        ...completeSources,
        errors: [
          { provider: "steam", code: "network", message: "recent unavailable" },
        ],
      },
      game,
    );
    expect(summary.sources.steam.status).toBe("available");
    expect(summary.sources.steam.error?.code).toBe("network");
    expect(summary.steamHistorical?.totalCount).toBe(100);
  });

  it("supports empty and invalid provider responses", () => {
    expect(parseGameReviewsSummary({}, game).status).toBe("no-data");
    expect(parseGameReviewsSummary(null, game).status).toBe("no-data");
    expect(
      parseGameReviewsSummary(
        {
          errors: [
            { provider: "steam", code: "invalid-response", message: null },
            { provider: "metacritic", code: "network", message: null },
            { provider: "opencritic", code: "network", message: null },
          ],
        },
        game,
      ).status,
    ).toBe("error");
  });

  it("reports an absent Steam identifier without making requests", () => {
    const missing = createIdentifierMissingSummary({
      id: "local-1",
      title: "Unknown game",
    });
    expect(missing.status).toBe("identifier-missing");
    expect(missing.errors[0]).toMatchObject({
      provider: "steam",
      code: "identifier-missing",
    });
  });
});
