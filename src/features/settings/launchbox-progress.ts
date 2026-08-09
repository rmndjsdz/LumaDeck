import type { LaunchBoxCatalogProgress } from "./provider-settings-service";

export type LaunchBoxPhaseCopy = {
  title: string;
  description: string;
};

export function launchBoxPhaseCopy(
  progress: LaunchBoxCatalogProgress | null,
): LaunchBoxPhaseCopy {
  switch (progress?.phase) {
    case "extracting":
      return {
        title: "Preparando catálogo",
        description: "Extrayendo archivos de metadatos…",
      };
    case "importing":
      return {
        title: "Importando metadatos",
        description: "Procesando juegos del catálogo local…",
      };
    case "validating":
      return {
        title: "Validando catálogo",
        description: "Comprobando la integridad de los datos importados…",
      };
    case "activating":
      return {
        title: "Activando catálogo",
        description: "Preparando la nueva versión para LumaDeck…",
      };
    case "downloading":
    default:
      return {
        title: "Descargando catálogo",
        description: "Descargando la fuente oficial de metadatos…",
      };
  }
}

export function launchBoxProgressPercent(
  progress: LaunchBoxCatalogProgress | null,
): number | null {
  if (!progress) return null;
  const total =
    progress.phase === "downloading"
      ? progress.totalBytes
      : progress.totalRecords;
  const value =
    progress.phase === "downloading"
      ? progress.downloadedBytes
      : progress.processedRecords;
  if (total === null || total === undefined || total <= 0) return null;
  return Math.min(100, Math.round(((value ?? 0) / total) * 100));
}

export function formatLaunchBoxDuration(milliseconds: number): string {
  const seconds = Math.max(0, Math.floor(milliseconds / 1000));
  const minutes = Math.floor(seconds / 60);
  return minutes > 0
    ? `${minutes}m ${String(seconds % 60).padStart(2, "0")}s`
    : `${seconds}s`;
}
