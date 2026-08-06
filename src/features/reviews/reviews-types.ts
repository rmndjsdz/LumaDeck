export type ReviewProvider = "metacritic" | "opencritic" | "steam";

export type ReviewsSummaryStatus =
  "success" | "partial" | "no-data" | "identifier-missing" | "error";

export type ReviewSourceStatus = "available" | "unavailable" | "error";

export type ReviewProviderErrorCode =
  | "network"
  | "timeout"
  | "cancelled"
  | "invalid-response"
  | "not-found"
  | "identifier-missing"
  | "runtime-unavailable"
  | "unknown";

export interface ReviewProviderError {
  provider: ReviewProvider;
  code: ReviewProviderErrorCode;
  message: string | null;
}

export interface ReviewDistribution {
  positiveCount: number | null;
  neutralCount: number | null;
  negativeCount: number | null;
  totalCount: number | null;
  positivePercent: number | null;
  neutralPercent: number | null;
  negativePercent: number | null;
}

export interface ReviewSourceSummary {
  provider: ReviewProvider;
  status: ReviewSourceStatus;
  score: number | null;
  platform?: string | null;
  maxScore: number;
  count: number | null;
  url: string | null;
  distribution: ReviewDistribution | null;
  error: ReviewProviderError | null;
}

export interface FeaturedReview {
  id: string;
  provider: "steam";
  author: string;
  text: string;
  recommended: boolean;
  playtimeHours: number | null;
  createdAt: string | null;
  language: string | null;
  helpfulVotes: number | null;
}

export interface GameReviewsSummary {
  gameId: string;
  title: string;
  steamAppId: number | null;
  status: ReviewsSummaryStatus;
  sources: Record<ReviewProvider, ReviewSourceSummary>;
  steamRecent: ReviewDistribution | null;
  steamHistorical: ReviewDistribution | null;
  featuredReviews: FeaturedReview[];
  errors: ReviewProviderError[];
  fetchedAt: string;
  inputFingerprint: string | null;
}

export interface MetacriticDto {
  score?: unknown;
  platform?: unknown;
  url?: unknown;
}

export interface OpenCriticDto {
  id?: unknown;
  name?: unknown;
  score?: unknown;
  reviewCount?: unknown;
  percentRecommended?: unknown;
  url?: unknown;
}

export interface SteamReviewSummaryDto {
  totalReviews?: unknown;
  totalPositive?: unknown;
  totalNegative?: unknown;
  reviewScore?: unknown;
  reviewScoreDesc?: unknown;
}

export interface SteamReviewDto {
  recommendationId?: unknown;
  author?: unknown;
  review?: unknown;
  votedUp?: unknown;
  playtimeForeverMinutes?: unknown;
  timestampCreated?: unknown;
  language?: unknown;
  votesUp?: unknown;
}

export interface SteamReviewsDto {
  all?: {
    querySummary?: SteamReviewSummaryDto;
    reviews?: SteamReviewDto[];
  };
  recent?: {
    querySummary?: SteamReviewSummaryDto;
    reviews?: SteamReviewDto[];
  };
}

export interface ReviewProviderErrorDto {
  provider?: unknown;
  code?: unknown;
  message?: unknown;
}

export interface ReviewSourcesDto {
  gameId?: unknown;
  title?: unknown;
  steamAppId?: unknown;
  metacritic?: MetacriticDto | null;
  opencritic?: OpenCriticDto | null;
  steam?: SteamReviewsDto | null;
  errors?: ReviewProviderErrorDto[];
  inputFingerprint?: unknown;
}

export interface ReviewsGame {
  id: string;
  title: string;
  details?: {
    steam?: {
      appId?: number | null;
    };
  };
}
