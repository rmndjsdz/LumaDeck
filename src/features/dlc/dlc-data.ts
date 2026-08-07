import type { Game } from "../catalog/game-types";

export type DlcContentType =
  "Historia" | "Pase de temporada" | "Misión" | "Contenido extra";

export type DlcStatus = "installed" | "available" | "owned";

export type DlcItem = {
  id: string;
  contentType: DlcContentType;
  title: string;
  shortDescription: string;
  description: string;
  releaseDate: string;
  status: DlcStatus;
  contextualAction: string;
  installationDate: string | null;
  size: string;
  version: string;
  platform: string;
  language: string;
  heroUrl: string;
};

const dlcTemplates: Array<
  Omit<DlcItem, "id" | "title" | "heroUrl" | "platform" | "language">
> = [
  {
    contentType: "Historia",
    shortDescription: "Una nueva ruta narrativa para el universo principal.",
    description:
      "Amplía la campaña con una aventura independiente, nuevos escenarios y decisiones que dejan huella en el mundo de la partida.",
    releaseDate: "14 jun 2026",
    status: "installed",
    contextualAction: "Gestionar",
    installationDate: "15 jun 2026",
    size: "8,4 GB",
    version: "1.4.2",
  },
  {
    contentType: "Pase de temporada",
    shortDescription: "Tres expansiones reunidas en una sola colección.",
    description:
      "El pase de temporada reúne los próximos capítulos, recompensas cosméticas y contenido adicional para continuar la experiencia.",
    releaseDate: "02 may 2026",
    status: "owned",
    contextualAction: "Ver contenido",
    installationDate: null,
    size: "—",
    version: "1.0.0",
  },
  {
    contentType: "Misión",
    shortDescription: "Un encargo opcional con un nuevo desafío táctico.",
    description:
      "Sigue una señal perdida en territorio hostil y desbloquea una misión secundaria con recompensas únicas.",
    releaseDate: "28 mar 2026",
    status: "available",
    contextualAction: "Instalar",
    installationDate: null,
    size: "2,1 GB",
    version: "1.2.0",
  },
  {
    contentType: "Contenido extra",
    shortDescription: "Arte conceptual y banda sonora original.",
    description:
      "Descubre el proceso creativo detrás del juego con una galería de arte y la banda sonora remasterizada.",
    releaseDate: "11 feb 2026",
    status: "available",
    contextualAction: "Ver contenido",
    installationDate: null,
    size: "640 MB",
    version: "1.0.0",
  },
];

const statusByIndex: DlcStatus[] = [
  "installed",
  "owned",
  "available",
  "available",
];

export function getGameDlc(game: Game): DlcItem[] {
  const steamDlcIds = game.details?.steam?.dlc ?? [];
  const sourceIds =
    steamDlcIds.length > 0
      ? steamDlcIds.slice(0, dlcTemplates.length)
      : [1, 2, 3, 4];
  const fallbackImages = [game.backgroundUrl, ...game.screenshots];
  const platform = game.details?.steam?.platforms?.join(" · ") || game.platform;
  const language = game.details?.steam?.languages?.[0] || "Español · Inglés";

  return sourceIds.map((sourceId, index) => {
    const template = dlcTemplates[index % dlcTemplates.length];
    const status = statusByIndex[index % statusByIndex.length];
    return {
      ...template,
      id: `dlc-${game.id}-${sourceId}`,
      title: `${game.title}: ${
        [
          "Horizonte Roto",
          "Frontier Pass",
          "Señal Perdida",
          "Archivo del Director",
        ][index % 4]
      }`,
      status,
      installationDate:
        status === "installed" ? template.installationDate : null,
      platform,
      language,
      heroUrl:
        fallbackImages[index % fallbackImages.length] ?? game.backgroundUrl,
    };
  });
}

export function dlcStatusLabel(status: DlcStatus): string {
  switch (status) {
    case "installed":
      return "Instalado";
    case "owned":
      return "Propiedad";
    case "available":
      return "Disponible";
  }
}

export function dlcStatusIcon(status: DlcStatus): string {
  switch (status) {
    case "installed":
      return "✓";
    case "owned":
      return "◆";
    case "available":
      return "＋";
  }
}
