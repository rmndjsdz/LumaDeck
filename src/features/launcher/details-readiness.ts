import type { Game } from "../catalog/game-types";

export type DetailsQueryStatus = "pending" | "error" | "success";
export type DetailsReadiness = "waiting" | "ready" | "unavailable";

export function getDetailsReadiness(
  data: Game | undefined,
  status: DetailsQueryStatus,
): DetailsReadiness {
  if (data) return "ready";
  return status === "pending" ? "waiting" : "unavailable";
}

export function shouldShowEmptyScreenshots(
  data: Game | undefined,
  screenshotCount: number,
): boolean {
  return Boolean(data && screenshotCount === 0);
}
