import { useQuery } from "@tanstack/react-query";
import { createMockCatalog } from "./mock-catalog";

export function useGames() {
  return useQuery({
    queryKey: ["games"],
    queryFn: async () => createMockCatalog(),
    staleTime: Infinity,
    gcTime: Infinity,
  });
}
