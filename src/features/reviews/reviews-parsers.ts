import type {
  FeaturedReview,
  GameReviewsSummary,
  MetacriticDto,
  OpenCriticDto,
  ReviewDistribution,
  ReviewProvider,
  ReviewProviderError,
  ReviewProviderErrorCode,
  ReviewSourceSummary,
  ReviewSourcesDto,
  ReviewsGame,
  SteamReviewDto,
  SteamReviewsDto,
  SteamReviewSummaryDto,
} from "./reviews-types";

const PROVIDERS: readonly ReviewProvider[] = [
  "metacritic",
  "opencritic",
  "steam",
];

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object";
}

function recordValue(record: Record<string, unknown>, key: string): unknown {
  return record[key];
}

function finiteNumber(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim() !== "") {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

function boundedScore(value: unknown): number | null {
  const score = finiteNumber(value);
  return score !== null && score >= 0 && score <= 100 ? score : null;
}

function nonNegativeInteger(value: unknown): number | null {
  const number = finiteNumber(value);
  return number !== null && number >= 0 ? Math.trunc(number) : null;
}

function text(value: unknown): string | null {
  return typeof value === "string" && value.trim() !== "" ? value.trim() : null;
}

function isSupportedReviewLanguage(value: unknown): boolean {
  const language = text(value)?.toLowerCase();
  return (
    language === "english" ||
    language === "en" ||
    language === "spanish" ||
    language === "es"
  );
}

function url(value: unknown): string | null {
  const candidate = text(value);
  if (!candidate) return null;
  try {
    const parsed = new URL(candidate);
    return parsed.protocol === "https:" || parsed.protocol === "http:"
      ? parsed.toString()
      : null;
  } catch {
    return null;
  }
}

function percent(count: number | null, total: number | null): number | null {
  if (count === null || total === null || total <= 0) return null;
  return Math.round((count / total) * 1000) / 10;
}

function distribution(
  positiveCount: number | null,
  neutralCount: number | null,
  negativeCount: number | null,
  totalCountOverride: number | null = null,
): ReviewDistribution | null {
  const counts = [positiveCount, neutralCount, negativeCount];
  if (counts.every((value) => value === null)) return null;
  const totalCount =
    totalCountOverride ??
    (counts.every((value) => value !== null)
      ? counts.reduce((sum, value) => sum + (value ?? 0), 0)
      : null);
  return {
    positiveCount,
    neutralCount,
    negativeCount,
    totalCount,
    positivePercent: percent(positiveCount, totalCount),
    neutralPercent: percent(neutralCount, totalCount),
    negativePercent: percent(negativeCount, totalCount),
  };
}

function providerError(
  provider: ReviewProvider,
  code: ReviewProviderErrorCode,
  message: string | null = null,
): ReviewProviderError {
  return { provider, code, message };
}

function providerErrorFromDto(value: unknown): ReviewProviderError | null {
  if (!isRecord(value)) return null;
  const providerValue = text(recordValue(value, "provider"));
  const codeValue = text(recordValue(value, "code"));
  if (
    !providerValue ||
    !PROVIDERS.includes(providerValue as ReviewProvider) ||
    !codeValue
  ) {
    return null;
  }
  const knownCodes: readonly ReviewProviderErrorCode[] = [
    "network",
    "timeout",
    "cancelled",
    "invalid-response",
    "not-found",
    "identifier-missing",
    "runtime-unavailable",
    "unknown",
  ];
  const code = knownCodes.includes(codeValue as ReviewProviderErrorCode)
    ? (codeValue as ReviewProviderErrorCode)
    : "unknown";
  return providerError(
    providerValue as ReviewProvider,
    code,
    text(recordValue(value, "message")),
  );
}

function emptySource(
  provider: ReviewProvider,
  error: ReviewProviderError | null = null,
): ReviewSourceSummary {
  return {
    provider,
    status: error ? "error" : "unavailable",
    score: null,
    platform: null,
    maxScore: 100,
    count: null,
    url: null,
    distribution: null,
    error,
  };
}

export function parseMetacritic(
  value: unknown,
  error: ReviewProviderError | null = null,
): ReviewSourceSummary {
  if (error) return emptySource("metacritic", error);
  if (!isRecord(value)) return emptySource("metacritic");
  const dto = value as MetacriticDto;
  const score = boundedScore(dto.score);
  const platform = text(dto.platform);
  const sourceUrl = url(dto.url);
  if (score === null && sourceUrl === null) return emptySource("metacritic");
  return {
    provider: "metacritic",
    status: score === null ? "unavailable" : "available",
    score,
    platform,
    maxScore: 100,
    count: null,
    url: sourceUrl,
    distribution: null,
    error: null,
  };
}

export function parseOpenCritic(
  value: unknown,
  error: ReviewProviderError | null = null,
): ReviewSourceSummary {
  if (error) return emptySource("opencritic", error);
  if (!isRecord(value)) return emptySource("opencritic");
  const dto = value as OpenCriticDto;
  const score = boundedScore(dto.score);
  const count = nonNegativeInteger(dto.reviewCount);
  const sourceUrl = url(dto.url);
  if (score === null && count === null && sourceUrl === null) {
    return emptySource("opencritic");
  }
  return {
    provider: "opencritic",
    status: score === null && count === null ? "unavailable" : "available",
    score,
    platform: null,
    maxScore: 100,
    count,
    url: sourceUrl,
    distribution: null,
    error: null,
  };
}

function parseSteamSummary(value: unknown): ReviewDistribution | null {
  if (!isRecord(value)) return null;
  const dto = value as SteamReviewSummaryDto;
  const positiveCount = nonNegativeInteger(dto.totalPositive);
  const negativeCount = nonNegativeInteger(dto.totalNegative);
  return distribution(
    positiveCount,
    0,
    negativeCount,
    nonNegativeInteger(dto.totalReviews),
  );
}

function parseSteamReview(
  value: unknown,
  index: number,
): FeaturedReview | null {
  if (!isRecord(value)) return null;
  const dto = value as SteamReviewDto;
  const textValue = text(dto.review);
  if (!textValue) return null;
  const createdSeconds = nonNegativeInteger(dto.timestampCreated);
  return {
    id: text(dto.recommendationId) ?? `steam-review-${index}`,
    provider: "steam",
    author: text(dto.author) ?? "Comunidad Steam",
    text: textValue,
    recommended: dto.votedUp === true,
    playtimeHours:
      nonNegativeInteger(dto.playtimeForeverMinutes) === null
        ? null
        : Math.round(
            ((nonNegativeInteger(dto.playtimeForeverMinutes) ?? 0) / 60) * 10,
          ) / 10,
    createdAt:
      createdSeconds === null
        ? null
        : new Date(createdSeconds * 1000).toISOString(),
    language: text(dto.language),
    helpfulVotes: nonNegativeInteger(dto.votesUp),
  };
}

export function parseSteam(
  value: unknown,
  error: ReviewProviderError | null = null,
): {
  source: ReviewSourceSummary;
  recent: ReviewDistribution | null;
  historical: ReviewDistribution | null;
  featured: FeaturedReview[];
} {
  if (error && !isRecord(value)) {
    return {
      source: emptySource("steam", error),
      recent: null,
      historical: null,
      featured: [],
    };
  }
  if (!isRecord(value)) {
    return {
      source: emptySource("steam"),
      recent: null,
      historical: null,
      featured: [],
    };
  }
  const dto = value as SteamReviewsDto;
  const all = isRecord(dto.all) ? dto.all : null;
  const recent = isRecord(dto.recent) ? dto.recent : null;
  const historical = parseSteamSummary(all?.querySummary);
  const recentDistribution = parseSteamSummary(recent?.querySummary);
  const reviews =
    Array.isArray(all?.reviews) && all.reviews.length > 0
      ? all.reviews
      : Array.isArray(recent?.reviews)
        ? recent.reviews
        : [];
  const featured = reviews
    .filter(
      (review) =>
        isRecord(review) && isSupportedReviewLanguage(review.language),
    )
    .map(parseSteamReview)
    .filter((review): review is FeaturedReview => review !== null)
    .slice(0, 6);
  const count =
    historical?.totalCount ?? recentDistribution?.totalCount ?? null;
  const score = historical
    ? historical.positivePercent
    : (recentDistribution?.positivePercent ?? null);
  const available = count !== null || score !== null || featured.length > 0;
  return {
    source: {
      provider: "steam",
      status: available ? "available" : error ? "error" : "unavailable",
      score,
      platform: null,
      maxScore: 100,
      count,
      url: null,
      distribution: historical,
      error,
    },
    recent: recentDistribution,
    historical,
    featured,
  };
}

function emptySummary(
  game: ReviewsGame,
  status: "identifier-missing" | "no-data",
): GameReviewsSummary {
  return {
    gameId: game.id,
    title: game.title,
    steamAppId: null,
    status,
    sources: {
      metacritic: emptySource("metacritic"),
      opencritic: emptySource("opencritic"),
      steam: emptySource("steam"),
    },
    steamRecent: null,
    steamHistorical: null,
    featuredReviews: [],
    errors: [],
    fetchedAt: new Date().toISOString(),
    inputFingerprint: null,
  };
}

export function parseGameReviewsSummary(
  value: unknown,
  game: ReviewsGame,
): GameReviewsSummary {
  const sourceDto: ReviewSourcesDto = isRecord(value)
    ? (value as ReviewSourcesDto)
    : {};
  const errors = (Array.isArray(sourceDto.errors) ? sourceDto.errors : [])
    .map(providerErrorFromDto)
    .filter((item): item is ReviewProviderError => item !== null);
  const errorFor = (provider: ReviewProvider) =>
    errors.find((item) => item.provider === provider) ?? null;
  const metacritic = parseMetacritic(
    sourceDto.metacritic,
    errorFor("metacritic"),
  );
  const opencritic = parseOpenCritic(
    sourceDto.opencritic,
    errorFor("opencritic"),
  );
  const steam = parseSteam(sourceDto.steam, errorFor("steam"));
  const sources = {
    metacritic,
    opencritic,
    steam: steam.source,
  } satisfies Record<ReviewProvider, ReviewSourceSummary>;
  const availableCount = Object.values(sources).filter(
    (source) => source.status === "available",
  ).length;
  const hasProviderErrors = errors.length > 0;
  const hasData = availableCount > 0 || steam.featured.length > 0;
  const status: GameReviewsSummary["status"] = !hasData
    ? hasProviderErrors
      ? "error"
      : "no-data"
    : availableCount === PROVIDERS.length && !hasProviderErrors
      ? "success"
      : "partial";
  return {
    gameId: text(sourceDto.gameId) ?? game.id,
    title: text(sourceDto.title) ?? game.title,
    steamAppId:
      nonNegativeInteger(sourceDto.steamAppId) ??
      game.details?.steam?.appId ??
      null,
    status,
    sources,
    steamRecent: steam.recent,
    steamHistorical: steam.historical,
    featuredReviews: steam.featured,
    errors,
    fetchedAt: new Date().toISOString(),
    inputFingerprint: text(sourceDto.inputFingerprint),
  };
}

export function createIdentifierMissingSummary(
  game: ReviewsGame,
): GameReviewsSummary {
  const summary = emptySummary(game, "identifier-missing");
  const error = providerError(
    "steam",
    "identifier-missing",
    "Steam app ID is missing",
  );
  return {
    ...summary,
    errors: [error],
    sources: {
      ...summary.sources,
      steam: emptySource("steam", error),
    },
  };
}
