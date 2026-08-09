import { describe, expect, it } from "vitest";
import { getGameBackgroundUrl, getGameBackgroundUrls } from "./game-media";

const artwork = {
  backgroundUrl: "",
  screenshots: ["screenshot"],
  coverUrl: "cover",
  verticalCoverUrl: "vertical-cover",
};

describe("game media", () => {
  it("falls back to screenshot and cover artwork when the hero is empty", () => {
    expect(getGameBackgroundUrl(artwork)).toBe("screenshot");
    expect(getGameBackgroundUrls(artwork)).toEqual([
      "screenshot",
      "cover",
      "vertical-cover",
    ]);
  });

  it("does not return duplicate or whitespace-only URLs", () => {
    expect(
      getGameBackgroundUrls({
        ...artwork,
        backgroundUrl: "  cover  ",
        screenshots: ["cover", "  "],
      }),
    ).toEqual(["  cover  ", "vertical-cover"]);
  });
});
