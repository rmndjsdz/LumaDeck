export type NewsCategory =
  | "official"
  | "update"
  | "event"
  | "community"
  | "media"
  | "dlc"
  | "maintenance"
  | "other";

export type TranslationStatus =
  "pending" | "translating" | "translated" | "failed" | "stale";

export type NewsFilter = "all" | "official" | "updates" | "community";

export interface NewsSyncState {
  providerId: string;
  gameId: string;
  lastSuccessfulSyncAt: string | null;
  lastAttemptAt: string | null;
  lastErrorCode: string | null;
  cursor: string | null;
  isStale: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface NewsItemViewModel {
  newsItemId: string;
  category: NewsCategory;
  sourceUrl: string;
  publishedAt: string;
  displayTitle: string;
  displaySummary: string | null;
  displayContent: string | null;
  displayLanguage: string;
  originalTitle: string;
  originalSummary: string | null;
  originalContent: string | null;
  contentFormat: "plain_text" | "html" | "markdown" | "unknown";
  imageUrl: string | null;
  thumbnailUrl: string | null;
  imageSource: string | null;
  commentCount: number | null;
  translationStatus: TranslationStatus | null;
  hasTranslation: boolean;
  sourceLanguage: string;
  targetLanguage: string | null;
}

export interface NewsFeedViewModel {
  gameId: string;
  syncState: NewsSyncState | null;
  isStale: boolean;
  totalCount: number;
  activeProvider: string;
  targetLanguage: string | null;
  hero: NewsItemViewModel | null;
  items: NewsItemViewModel[];
  secondaryItems: NewsItemViewModel[];
  availableCategories: NewsCategory[];
  warnings: string[];
}

export interface NewsRefreshResult {
  providerId: string;
  gameId: string;
  steamAppId: number;
  fetchedCount: number;
  acceptedCount: number;
  discardedCount: number;
  deduplicatedCount: number;
  insertedCount: number;
  updatedCount: number;
  unchangedCount: number;
  skippedDueToFreshness: boolean;
  lastSuccessfulSyncAt: string | null;
  warnings: string[];
}

export interface TranslationBatchSummary {
  targetLanguage: string;
  requestedCount: number;
  cacheHitCount: number;
  translatedCount: number;
  failedCount: number;
  partialFailure: boolean;
}

export const NEWS_FILTERS: ReadonlyArray<{
  id: NewsFilter;
  label: string;
  categories: NewsCategory[];
}> = [
  { id: "all", label: "Todas", categories: [] },
  { id: "official", label: "Oficiales", categories: ["official"] },
  {
    id: "updates",
    label: "Actualizaciones",
    categories: ["update", "event", "dlc", "maintenance"],
  },
  { id: "community", label: "Comunidad", categories: ["community"] },
];

export function filterCategories(filter: NewsFilter): NewsCategory[] {
  return (
    NEWS_FILTERS.find((candidate) => candidate.id === filter)?.categories ?? []
  );
}

export function categoryLabel(category: NewsCategory): string {
  switch (category) {
    case "official":
      return "OFICIAL";
    case "update":
      return "ACTUALIZACIÓN";
    case "event":
      return "EVENTO";
    case "community":
      return "COMUNIDAD";
    case "dlc":
      return "DLC";
    case "maintenance":
      return "MANTENIMIENTO";
    case "media":
      return "MEDIOS";
    default:
      return "NOTICIA";
  }
}

export function formatNewsDate(value: string): string {
  const numericValue = Number(value);
  const date = Number.isFinite(numericValue)
    ? new Date(
        numericValue < 100_000_000_000 ? numericValue * 1000 : numericValue,
      )
    : new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("es-419", {
    day: "2-digit",
    month: "short",
    year: "numeric",
  })
    .format(date)
    .replace(".", "")
    .toUpperCase();
}
