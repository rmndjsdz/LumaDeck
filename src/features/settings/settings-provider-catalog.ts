import type { ActionAvailability } from "../../ui/navigation/actions/availability-types";

export const PROVIDERS = [
  ["eden", "Eden", "Nintendo Switch - discovery local de juegos", "E"],
  ["launchbox", "LaunchBox", "Metadatos y capturas para juegos emulados", "L"],
  ["steam", "Steam", "Sincroniza tu biblioteca y progreso", "◉"],
  ["hltb", "HowLongToBeat", "Duraciones estimadas para tus juegos", "H"],
  ["steamgriddb", "SteamGridDB", "Arte para personalizar tu biblioteca", "▪"],
  [
    "rapidapi-reviews",
    "OpenCritic / Metacritic",
    "Puntuaciones y reseñas de críticos",
    "R",
  ],
  [
    "ai-services",
    "Servicios IA",
    "Los Servicios IA permiten habilitar funciones avanzadas como consenso de reseñas, resúmenes de noticias, recomendaciones y futuras capacidades inteligentes de LumaDeck.",
    "✦",
  ],
  ["lossless-scaling", "Lossless Scaling", "Frame Generation por juego", "F"],
  ["epic", "Epic Games", "Sincroniza tu biblioteca", "E"],
  ["xbox", "Xbox", "Sincroniza logros y actividad", "X"],
  ["playstation", "PlayStation Network", "Sincroniza trofeos y juegos", "P"],
  ["ubisoft", "Ubisoft Connect", "Sincroniza tu biblioteca", "U"],
  ["gog", "GOG Galaxy", "Sincroniza tu biblioteca", "G"],
] as const;

export function providerAvailability(id: string): ActionAvailability {
  return id === "steam" ||
    id === "eden" ||
    id === "launchbox" ||
    id === "hltb" ||
    id === "steamgriddb" ||
    id === "rapidapi-reviews" ||
    id === "ai-services" ||
    id === "lossless-scaling"
    ? "available"
    : "coming-soon";
}
