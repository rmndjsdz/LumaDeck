import type { Game, GameStatus } from "../catalog/game-types";
import { getVisibleGames } from "../catalog/game-visibility";
import type { LibrarySort } from "../../stores/library-store";

export type { LibrarySort } from "../../stores/library-store";

export type LibraryGenreId =
  | "all"
  | "local-multiplayer"
  | "fighting"
  | "beat-em-up"
  | "sports"
  | "racing"
  | "party"
  | "action"
  | "adventure"
  | "rpg"
  | "strategy"
  | "open-world"
  | "sandbox"
  | "puzzle"
  | "simulation"
  | "indie"
  | "horror"
  | "platformer"
  | "roguelike"
  | "visual-novel";

export interface LibraryGenreFilter {
  id: LibraryGenreId;
  label: string;
  icon: string;
}

export const LIBRARY_PRIMARY_GENRE_FILTERS: readonly LibraryGenreFilter[] = [
  { id: "all", label: "Todos", icon: "▦" },
  { id: "local-multiplayer", label: "Local Multiplayer", icon: "♟" },
  { id: "fighting", label: "Fighting", icon: "⚔" },
  { id: "beat-em-up", label: "Beat 'em up", icon: "✺" },
  { id: "sports", label: "Deportes", icon: "◉" },
  { id: "racing", label: "Carreras", icon: "⚑" },
  { id: "party", label: "Party", icon: "✣" },
  { id: "action", label: "Acción", icon: "✷" },
  { id: "adventure", label: "Aventura", icon: "◢" },
  { id: "rpg", label: "RPG", icon: "♧" },
];

export const LIBRARY_MORE_GENRE_FILTERS: readonly LibraryGenreFilter[] = [
  { id: "strategy", label: "Estrategia", icon: "⌁" },
  { id: "open-world", label: "Open World", icon: "◉" },
  { id: "simulation", label: "Simulación", icon: "◌" },
  { id: "indie", label: "Indie", icon: "✦" },
  { id: "horror", label: "Horror", icon: "☾" },
  { id: "platformer", label: "Platformer", icon: "↗" },
  { id: "roguelike", label: "Roguelike", icon: "◇" },
  { id: "visual-novel", label: "Visual Novel", icon: "▤" },
];

const GENRE_ALIASES: Readonly<Record<string, LibraryGenreId>> = {
  fighting: "fighting",
  fighter: "fighting",
  fightinggame: "fighting",
  fightinggames: "fighting",
  "2dfighter": "fighting",
  "3dfighter": "fighting",
  arenafighter: "fighting",
  platformfighter: "fighting",
  versusfighter: "fighting",
  beatemup: "beat-em-up",
  brawler: "beat-em-up",
  sports: "sports",
  sport: "sports",
  racing: "racing",
  racesim: "racing",
  party: "party",
  action: "action",
  adventure: "adventure",
  rpg: "rpg",
  roleplaying: "rpg",
  strategy: "strategy",
  openworld: "open-world",
  openworldgame: "open-world",
  sandbox: "sandbox",
  puzzle: "puzzle",
  simulation: "simulation",
  indie: "indie",
  horror: "horror",
  platformer: "platformer",
  roguelike: "roguelike",
  visualnovel: "visual-novel",
};

const LOCAL_MULTIPLAYER_SIGNALS = [
  "localmultiplayer",
  "localcoop",
  "couchcoop",
  "splitscreen",
  "sharedscreen",
  "sharedsplitscreen",
] as const;

const TITLE_GENRE_SIGNALS: readonly {
  genre: LibraryGenreId;
  terms: readonly string[];
}[] = [
  {
    genre: "fighting",
    terms: [
      "fighting",
      "fighter",
      "tekken",
      "mortalkombat",
      "guiltygear",
      "streetfighter",
      "dragonballfighterz",
      "marvelvscapcom",
      "killerinstinct",
      "soulcalibur",
      "deadordead",
      "injustice",
      "kingoffighters",
      "blazblue",
      "samuraishodown",
      "skullgirls",
      "meltyblood",
    ],
  },
  {
    genre: "beat-em-up",
    terms: [
      "beatemup",
      "brawler",
      "streetsofrage",
      "shreddersrevenge",
      "marvelcosmicinvasion",
      "battletoads",
      "finalfight",
      "rivercitygirls",
      "doubledragon",
      "scottpilgrim",
      "castlecrashers",
    ],
  },
  {
    genre: "open-world",
    terms: [
      "openworld",
      "thewitcher3",
      "thewitcher",
      "grandtheftauto",
      "gta",
      "reddeadredemption",
      "crimsondesert",
      "cyberpunk2077",
      "skyrim",
      "fallout",
      "eldenring",
      "horizonzerodawn",
      "horizonforbiddenwest",
      "ghostoftsushima",
      "watchdogs",
      "farcry",
      "justcause",
      "deathstranding",
    ],
  },
  {
    genre: "horror",
    terms: [
      "horror",
      "residentevil",
      "silenthill",
      "deadspace",
      "theevilwithin",
      "outlast",
      "amnesia",
      "soma",
      "alienisolation",
      "untildawn",
      "littlenightmares",
      "theforest",
      "phasmaphobia",
      "layersoffear",
      "visage",
    ],
  },
];

export function normalizeLibraryGenre(value: string): string {
  return value
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLocaleLowerCase()
    .replace(/[^a-z0-9]/g, "");
}

export function getLibraryGenreIds(game: Game): LibraryGenreId[] {
  const metadata = collectGenreMetadata(game);
  const genreIds = new Set<LibraryGenreId>();
  const normalizedTitle = normalizeLibraryGenre(game.title);

  for (const value of metadata) {
    const normalized = normalizeLibraryGenre(value);
    const genreId =
      GENRE_ALIASES[normalized] ??
      (normalized.includes("fighter") || normalized.includes("fighting")
        ? "fighting"
        : normalized.includes("beatemup") || normalized.includes("brawler")
          ? "beat-em-up"
          : undefined);
    if (genreId) genreIds.add(genreId);
  }

  // The current local Steam snapshot exposes these games as Action only and
  // has no populated tags table, so use conservative title signals as a
  // temporary genre fallback until richer provider metadata is available.
  for (const { genre, terms } of TITLE_GENRE_SIGNALS) {
    if (terms.some((term) => normalizedTitle.includes(term))) {
      genreIds.add(genre);
    }
  }

  if (
    metadata.some((value) => {
      const normalized = normalizeLibraryGenre(value);
      return LOCAL_MULTIPLAYER_SIGNALS.some((signal) =>
        normalized.includes(signal),
      );
    })
  ) {
    genreIds.add("local-multiplayer");
  }

  if (game.details?.launchbox?.localMultiplayer === "true") {
    genreIds.add("local-multiplayer");
  }

  return [...genreIds];
}

export function matchesLibraryGenre(
  game: Game,
  genre: LibraryGenreId,
): boolean {
  if (genre === "all") return true;
  const genreIds = getLibraryGenreIds(game);
  return (
    genreIds.includes(genre) ||
    (genre === "open-world" && genreIds.includes("sandbox"))
  );
}

function collectGenreMetadata(game: Game): string[] {
  const steam = game.details?.steam;
  const launchbox = game.details?.launchbox;
  return [
    ...game.genres,
    ...(launchbox?.normalizedGenres ?? []),
    ...(steam?.genres ?? []),
    ...(steam?.tags ?? []),
    ...(steam?.categories ?? []),
  ].filter(Boolean);
}

export function filterAndSortGames(
  games: readonly Game[],
  query: string,
  status: "all" | GameStatus,
  sort: LibrarySort,
  genre: LibraryGenreId = "all",
): Game[] {
  const normalized = query.trim().toLocaleLowerCase();
  return getVisibleGames(games)
    .filter(
      (game) =>
        !normalized || game.title.toLocaleLowerCase().includes(normalized),
    )
    .filter((game) => status === "all" || game.status === status)
    .filter((game) => matchesLibraryGenre(game, genre))
    .sort((left, right) => {
      if (sort === "recent")
        return (right.lastPlayedAt ?? "").localeCompare(
          left.lastPlayedAt ?? "",
        );
      if (sort === "time") return right.playtimeMinutes - left.playtimeMinutes;
      return left.sortTitle.localeCompare(right.sortTitle);
    });
}
