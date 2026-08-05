import { useQuery, useQueryClient } from "@tanstack/react-query";
import { achievementsService } from "./achievement-service";

const ACHIEVEMENT_STALE_TIME = 5 * 60 * 1000;
const ACHIEVEMENT_AUTO_REFRESH_INTERVAL = 15 * 60 * 1000;

export function useAchievements(gameId: string | undefined) {
  return useQuery({
    queryKey: ["achievements", gameId],
    queryFn: () => achievementsService.getGameAchievements(gameId ?? ""),
    enabled: Boolean(gameId),
    staleTime: ACHIEVEMENT_STALE_TIME,
  });
}

export function useAchievementSummary(gameId: string | undefined) {
  return useQuery({
    queryKey: ["achievements", "summary", gameId],
    queryFn: () => achievementsService.getAchievementSummary(gameId ?? ""),
    enabled: Boolean(gameId),
    staleTime: ACHIEVEMENT_STALE_TIME,
  });
}

export function useAchievementDistribution(gameId: string | undefined) {
  return useQuery({
    queryKey: ["achievements", "distribution", gameId],
    queryFn: () => achievementsService.getAchievementDistribution(gameId ?? ""),
    enabled: Boolean(gameId),
    staleTime: ACHIEVEMENT_STALE_TIME,
  });
}

export function useAchievementRecent(gameId: string | undefined) {
  const query = useAchievements(gameId);
  return {
    ...query,
    data: query.data?.recent,
  };
}

export function useRefreshGameAchievements() {
  const queryClient = useQueryClient();
  return async (gameId: string) => {
    const result = await achievementsService.refreshGameAchievements(gameId);
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["achievements", gameId] }),
      queryClient.invalidateQueries({
        queryKey: ["achievements", "summary", gameId],
      }),
      queryClient.invalidateQueries({
        queryKey: ["achievements", "distribution", gameId],
      }),
    ]);
    return result;
  };
}

export function useAutoRefreshGameAchievements(gameId: string | undefined) {
  const queryClient = useQueryClient();
  return useQuery({
    queryKey: ["achievements", "auto-refresh", gameId],
    queryFn: async () => {
      const result = await achievementsService.refreshGameAchievements(
        gameId ?? "",
      );
      queryClient.setQueryData(["achievements", gameId], result);
      queryClient.setQueryData(
        ["achievements", "summary", gameId],
        result.summary,
      );
      queryClient.setQueryData(
        ["achievements", "distribution", gameId],
        result.distribution,
      );
      return result;
    },
    enabled: Boolean(gameId),
    retry: false,
    staleTime: ACHIEVEMENT_AUTO_REFRESH_INTERVAL,
    refetchInterval: ACHIEVEMENT_AUTO_REFRESH_INTERVAL,
    refetchOnWindowFocus: true,
  });
}
