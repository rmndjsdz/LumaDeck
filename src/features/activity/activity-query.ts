import { useQuery } from "@tanstack/react-query";
import { activityService } from "./activity-service";

const ACTIVITY_STALE_TIME = 5 * 60 * 1000;
const ACTIVITY_CACHE_TIME = 30 * 60 * 1000;

export function useActivity(gameId: string | undefined) {
  return useQuery({
    queryKey: ["activity", gameId],
    queryFn: () => activityService.get(gameId ?? ""),
    enabled: Boolean(gameId),
    staleTime: ACTIVITY_STALE_TIME,
    gcTime: ACTIVITY_CACHE_TIME,
    placeholderData: (previousData) => previousData,
  });
}
