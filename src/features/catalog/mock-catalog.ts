import type { Game, GameStatus } from "./game-types";

const titles = [
  "Aether Circuit",
  "Beacon Nine",
  "Cinder Vale",
  "Drift Protocol",
  "Echo Meridian",
  "Frostline",
  "Glass Horizon",
  "Helio Run",
  "Ion Garden",
  "Juniper Signal",
];
const genres = ["Action", "Adventure", "RPG", "Strategy", "Puzzle"];
const platforms = ["Windows", "Linux", "Steam Deck"];
const providers = ["Local", "Indie Vault", "Luma Picks"];
const statuses: GameStatus[] = ["not-started", "playing", "completed"];

function svgData(svg: string): string {
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

function createCover(index: number, title: string): string {
  const hue = (index * 37) % 360;
  return svgData(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 480 640">
    <defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop stop-color="hsl(${hue} 65% 35%)"/><stop offset="1" stop-color="hsl(${(hue + 70) % 360} 70% 16%)"/></linearGradient></defs>
    <rect width="480" height="640" fill="url(#g)"/><circle cx="380" cy="120" r="130" fill="white" opacity=".08"/><path d="M0 500 180 330 300 430 480 250V640H0Z" fill="white" opacity=".09"/><text x="32" y="560" fill="white" font-family="sans-serif" font-size="30" font-weight="700">${title}</text><text x="34" y="598" fill="white" opacity=".62" font-family="sans-serif" font-size="14">LUMADECK / ${String(index + 1).padStart(3, "0")}</text>
  </svg>`);
}

function createBackground(index: number): string {
  const hue = (index * 37) % 360;
  return svgData(
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1600 900"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop stop-color="hsl(${hue} 60% 16%)"/><stop offset=".55" stop-color="hsl(${(hue + 45) % 360} 50% 9%)"/><stop offset="1" stop-color="#07101e"/></linearGradient></defs><rect width="1600" height="900" fill="url(#g)"/><circle cx="1250" cy="170" r="310" fill="hsl(${(hue + 85) % 360} 80% 60%)" opacity=".08"/><path d="M0 740 520 420 920 650 1600 250V900H0Z" fill="#fff" opacity=".035"/></svg>`,
  );
}

export function createMockCatalog(): Game[] {
  return Array.from({ length: 200 }, (_, index) => {
    const baseTitle = titles[index % titles.length];
    const title = `${baseTitle} ${String(Math.floor(index / titles.length) + 1).padStart(2, "0")}`;
    const status = statuses[index % statuses.length];
    return {
      id: `game-${String(index + 1).padStart(3, "0")}`,
      title,
      sortTitle: title.toLocaleLowerCase(),
      platform: platforms[index % platforms.length],
      provider: providers[index % providers.length],
      coverUrl: createCover(index, title),
      backgroundUrl: createBackground(index),
      description: `${title} is a local LumaDeck catalog entry built for fast, reliable navigation.`,
      genres: [
        genres[index % genres.length],
        genres[(index + 2) % genres.length],
      ],
      releaseYear: 2016 + (index % 10),
      playtimeMinutes: (index * 47) % 240,
      lastPlayedAt:
        status === "not-started"
          ? null
          : `2026-07-${String((index % 28) + 1).padStart(2, "0")}`,
      favorite: index % 7 === 0,
      installed: index % 4 !== 0,
      progress:
        status === "not-started"
          ? 0
          : status === "completed"
            ? 100
            : 24 + (index % 65),
      status,
    } satisfies Game;
  });
}
