import { describe, expect, it } from "vitest";
import type { Game } from "../catalog/game-types";
import {
  DEFAULT_RECOMMENDATION_WEIGHTS,
  rankRecommendations,
} from "./recommendation-engine";

function game(overrides: Partial<Game> = {}): Game {
  return {
    id: "source",
    title: "Open World Ronin",
    sortTitle: "open world ronin",
    platform: "Windows",
    provider: "Local",
    coverUrl: "cover",
    verticalCoverUrl: "vertical-cover",
    logoUrl: "logo",
    backgroundUrl: "background",
    screenshots: [],
    description: "description",
    genres: ["Action", "Adventure"],
    releaseYear: 2020,
    playtimeMinutes: 1_500,
    lastPlayedAt: null,
    favorite: true,
    installed: true,
    progress: 42,
    status: "playing",
    ...overrides,
  };
}

describe("rankRecommendations", () => {
  it("excludes the source game and returns a descending composite score", () => {
    const source = game();
    const result = rankRecommendations(source, [
      source,
      game({ id: "weak", title: "Puzzle Garden", genres: ["Puzzle"] }),
      game({
        id: "strong",
        title: "Open World Ronin II",
        genres: ["Action", "Adventure"],
        favorite: true,
      }),
    ]);

    expect(result.map((recommendation) => recommendation.game.id)).toEqual([
      "strong",
      "weak",
    ]);
    expect(result[0]?.score).toBeGreaterThan(result[1]?.score ?? 0);
    expect(result[0]?.reasons.length).toBeGreaterThan(0);
  });

  it("accepts configurable signal weights", () => {
    const source = game();
    const weights = {
      ...DEFAULT_RECOMMENDATION_WEIGHTS,
      genres: 1,
      quality: 0,
    };
    const result = rankRecommendations(
      source,
      [
        game({ id: "same-genre" }),
        game({ id: "different", genres: ["Puzzle"] }),
      ],
      weights,
    );

    expect(result[0]?.game.id).toBe("same-genre");
    expect(result[0]?.reasons[0]?.signal).toBe("genres");
  });
});
