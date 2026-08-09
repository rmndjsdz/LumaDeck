import { describe, expect, it } from "vitest";
import type { Game } from "../catalog/game-types";
import { resolveFeaturedGame } from "./home-feature-selection";

function game(id: string, status: Game["status"] = "playing"): Game {
  return {
    id,
    title: id,
    sortTitle: id,
    platform: "PC",
    provider: "Steam",
    coverUrl: "cover",
    verticalCoverUrl: "cover",
    logoUrl: "logo",
    backgroundUrl: "background",
    screenshots: [],
    description: "",
    genres: [],
    releaseYear: 2026,
    playtimeMinutes: 60,
    lastPlayedAt: status === "not-started" ? null : "2026-07-28",
    favorite: false,
    installed: true,
    progress: 25,
    status,
  };
}

describe("resolveFeaturedGame", () => {
  it("follows the focused game across Home rows", () => {
    const first = game("game-001");
    const second = game("game-002");
    const games = [first, second];

    expect(resolveFeaturedGame(games, "home-continue-game-002")).toBe(second);
    expect(resolveFeaturedGame(games, "home-recent-game-002")).toBe(second);
  });

  it("returns the current object after its achievement data changes", () => {
    const first = game("game-001");
    const updated = {
      ...game("game-002"),
      achievements: { total: 49, unlocked: 11, progress: 22.4 },
    };

    expect(
      resolveFeaturedGame([first, updated], "home-continue-game-002")
        ?.achievements,
    ).toEqual({ total: 49, unlocked: 11, progress: 22.4 });
  });

  it("keeps the first Continue Playing game when focus is outside a row", () => {
    const first = game("game-001");
    const second = game("game-002");

    expect(resolveFeaturedGame([first, second], "main-nav-home")).toBe(first);
  });

  it("never selects a hidden game for the Home hero", () => {
    const hidden = { ...game("game-hidden"), hidden: true };
    const visible = game("game-visible");

    expect(resolveFeaturedGame([hidden, visible], "main-nav-home")).toBe(
      visible,
    );
  });
});
