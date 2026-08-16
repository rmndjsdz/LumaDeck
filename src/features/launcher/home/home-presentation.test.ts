import { describe, expect, it } from "vitest";
import type { Game } from "../../catalog/game-types";
import { buildHomePresentation } from "./home-presentation";

function makeGame(id: string, index: number): Game {
  return {
    id,
    title: id,
    sortTitle: id,
    platform: "Steam",
    provider: "Steam",
    coverUrl: `cover-${id}`,
    verticalCoverUrl: `vertical-${id}`,
    squareCoverUrl: `square-${id}`,
    logoUrl: `logo-${id}`,
    backgroundUrl: `background-${id}`,
    screenshots: [],
    description: "",
    genres: ["Adventure"],
    releaseYear: 2026,
    playtimeMinutes: 60 + index,
    lastPlayedAt: `2026-07-${String((index % 28) + 1).padStart(2, "0")}`,
    favorite: index % 4 === 0,
    installed: true,
    progress: 10,
    status: "playing",
  };
}

describe("Home presentation model", () => {
  it("keeps the cinematic rail bounded to twenty games and updates focus", () => {
    const games = Array.from({ length: 40 }, (_, index) =>
      makeGame(`game-${index}`, index),
    );

    const presentation = buildHomePresentation(games, "home-cinematic-game-11");

    expect(presentation.railGames).toHaveLength(20);
    expect(presentation.focusedGame?.id).toBe("game-11");
  });
});
