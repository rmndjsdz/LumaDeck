import type { Game } from "./game-types";

type GameArtwork = Pick<
  Game,
  "backgroundUrl" | "screenshots" | "coverUrl" | "verticalCoverUrl"
>;

export function getGameBackgroundUrls(game: GameArtwork): string[] {
  const seen = new Set<string>();
  return [
    game.backgroundUrl,
    game.screenshots[0] ?? "",
    game.coverUrl,
    game.verticalCoverUrl,
  ].filter((url) => {
    const normalizedUrl = url.trim();
    if (normalizedUrl.length === 0 || seen.has(normalizedUrl)) return false;
    seen.add(normalizedUrl);
    return true;
  });
}

export function getGameBackgroundUrl(game: GameArtwork): string {
  return getGameBackgroundUrls(game)[0] ?? "";
}
