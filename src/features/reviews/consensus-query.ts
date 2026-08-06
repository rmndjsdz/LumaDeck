import { useQuery } from "@tanstack/react-query";
import { providerSettingsService } from "../settings/provider-settings-service";
import { consensusService } from "./consensus-service";
import type { ReviewConsensusQueryData } from "./consensus-types";

export function reviewConsensusQueryKey(gameId: string | undefined) {
  return ["reviews-consensus", gameId] as const;
}

export function useGameReviewConsensus(gameId: string | undefined) {
  return useQuery<ReviewConsensusQueryData>({
    queryKey: reviewConsensusQueryKey(gameId),
    queryFn: async () => {
      if (
        !gameId ||
        typeof window === "undefined" ||
        !("__TAURI_INTERNALS__" in window)
      ) {
        return { consensus: null, aiConfigured: false };
      }
      const [consensus, configuration] = await Promise.all([
        consensusService.get(gameId),
        providerSettingsService.getAIConfiguration(),
      ]);
      return {
        consensus,
        aiConfigured:
          configuration.apiKeyConfigured && configuration.credentialAvailable,
      };
    },
    enabled: Boolean(gameId),
    staleTime: Number.POSITIVE_INFINITY,
    gcTime: 24 * 60 * 60 * 1000,
    retry: false,
  });
}
