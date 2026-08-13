import { invoke } from "@tauri-apps/api/core";
import type { NvidiaOpsProfile, NvidiaOpsResponse } from "./nvidia-ops-types";

export const nvidiaOpsService = {
  get(
    gameId: string,
    steamAppId: number | null,
    title: string | null,
    executablePath: string | null,
    displayResolution: { width: number; height: number } | null,
  ): Promise<NvidiaOpsResponse> {
    return invoke<NvidiaOpsResponse>("get_nvidia_ops_profile", {
      request: {
        gameId,
        steamAppId,
        title,
        executablePath,
        displayResolution,
      },
    });
  },
};

export function hasUsableNvidiaOpsProfile(
  response: NvidiaOpsResponse,
): response is NvidiaOpsResponse & {
  status: "AVAILABLE" | "BELOW_MIN_SPEC";
  profile: NvidiaOpsProfile;
} {
  return (
    (response.status === "AVAILABLE" || response.status === "BELOW_MIN_SPEC") &&
    response.profile !== null
  );
}
