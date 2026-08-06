import { useQuery } from "@tanstack/react-query";
import { reviewsService } from "./reviews-service";
import type { ReviewsGame } from "./reviews-types";

const REVIEWS_STALE_TIME = 30 * 60 * 1000;
const REVIEWS_GC_TIME = 24 * 60 * 60 * 1000;

export function reviewsQueryKey(gameId: string | undefined) {
  return ["reviews-summary", gameId] as const;
}

export function useGameReviewsSummary(game: ReviewsGame | undefined) {
  return useQuery({
    queryKey: reviewsQueryKey(game?.id),
    queryFn: ({ signal }) =>
      reviewsService.getGameReviewsSummary(
        game ?? { id: "", title: "" },
        signal,
      ),
    enabled: Boolean(game?.id),
    staleTime: REVIEWS_STALE_TIME,
    gcTime: REVIEWS_GC_TIME,
    retry: false,
  });
}
