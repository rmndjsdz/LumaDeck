import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useRef } from "react";
import { newsService } from "./news-service";
import type { NewsFeedViewModel, NewsFilter } from "./news-types";

const NEWS_DAY_MS = 24 * 60 * 60_000;
const NEWS_STALE_TIME = NEWS_DAY_MS;
const NEWS_CACHE_TIME = NEWS_DAY_MS;

export function newsFeedQueryKey(
  gameId: string | undefined,
  filter: NewsFilter,
) {
  return ["news-feed", gameId, filter, "es-419"] as const;
}

export function useNewsFeed(gameId: string | undefined, filter: NewsFilter) {
  return useQuery({
    queryKey: newsFeedQueryKey(gameId, filter),
    queryFn: () => newsService.getFeed(gameId ?? "", filter),
    enabled: Boolean(gameId),
    staleTime: NEWS_STALE_TIME,
    gcTime: NEWS_CACHE_TIME,
    placeholderData: (previousData) => previousData,
  });
}

export function useNewsSyncState(gameId: string | undefined) {
  return useQuery({
    queryKey: ["news-sync-state", gameId],
    queryFn: () => newsService.getSyncState(gameId ?? ""),
    enabled: Boolean(gameId),
    staleTime: NEWS_STALE_TIME,
    gcTime: NEWS_CACHE_TIME,
  });
}

export function useRefreshGameNews(gameId: string, filter: NewsFilter) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => newsService.refresh(gameId),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["news-feed", gameId] }),
        queryClient.invalidateQueries({
          queryKey: ["news-sync-state", gameId],
        }),
      ]);
    },
    meta: { filter },
  });
}

export function useTranslateVisibleNews(feed: NewsFeedViewModel | undefined) {
  const queryClient = useQueryClient();
  const requestedIdsRef = useRef("");
  const mutation = useMutation({
    mutationFn: (newsItemIds: string[]) => newsService.translate(newsItemIds),
    onSuccess: async () => {
      if (!feed) return;
      await queryClient.invalidateQueries({
        queryKey: ["news-feed", feed.gameId],
      });
    },
  });

  const visibleIds = useMemo(
    () =>
      feed
        ? [feed.hero, ...feed.items, ...feed.secondaryItems]
            .filter((item): item is NonNullable<typeof item> => item !== null)
            .map((item) => item.newsItemId)
        : [],
    [feed],
  );
  const visibleKey = visibleIds.join("|");

  useEffect(() => {
    if (
      !feed ||
      visibleIds.length === 0 ||
      mutation.isPending ||
      requestedIdsRef.current === visibleKey
    ) {
      return;
    }
    requestedIdsRef.current = visibleKey;
    mutation.mutate(visibleIds);
  }, [feed, mutation, visibleIds, visibleKey]);

  return mutation;
}
