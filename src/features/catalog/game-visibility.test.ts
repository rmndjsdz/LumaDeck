import { describe, expect, it } from "vitest";
import type { Game } from "./game-types";
import { getVisibleGames, isGameHidden } from "./game-visibility";

function game(id: string, hidden = false): Game {
  return {
    id,
    title: id,
    sortTitle: id,
    platform: "PC",
    provider: "Local",
    coverUrl: "cover",
    verticalCoverUrl: "cover",
    logoUrl: "logo",
    backgroundUrl: "background",
    screenshots: [],
    description: "",
    genres: [],
    releaseYear: 2026,
    playtimeMinutes: 30,
    lastPlayedAt: null,
    favorite: true,
    hidden,
    installed: true,
    progress: 20,
    status: "not-started",
  };
}

describe("game visibility", () => {
  it("excludes hidden games while keeping their game data intact", () => {
    const hidden = game("hidden", true);
    const visible = game("visible");

    expect(isGameHidden(hidden)).toBe(true);
    expect(getVisibleGames([hidden, visible])).toEqual([visible]);
    expect(hidden.favorite).toBe(true);
  });

  it("treats omitted hidden state as visible for older/local fixtures", () => {
    const legacy = game("legacy");
    delete legacy.hidden;

    expect(isGameHidden(legacy)).toBe(false);
    expect(getVisibleGames([legacy])).toEqual([legacy]);
  });
});
