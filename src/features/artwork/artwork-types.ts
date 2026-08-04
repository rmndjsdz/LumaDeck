export type ArtworkKind = "grid" | "hero" | "logo" | "icon";

export type ArtworkSlot =
  | "grid_horizontal"
  | "grid_vertical"
  | "grid_square"
  | "hero"
  | "logo"
  | "icon";

export type GridStyle = string;

export type ArtworkFilterKind = "all" | "no_logo" | "other";

export type ArtworkSearchRequest = {
  gameId: string;
  slot: ArtworkSlot;
  styleFilter: ArtworkFilterKind;
};

export type ArtworkPreviewCandidate = {
  candidateId: string;
  externalAssetId: number;
  externalGameId: number;
  kind: ArtworkKind;
  slot: ArtworkSlot;
  gridStyle: GridStyle | null;
  width: number;
  height: number;
  aspectRatio: number;
  thumbnailUrl: string;
  mimeType: string | null;
  score: number | null;
  upvotes: number | null;
  downvotes: number | null;
  nsfw: boolean;
  locked: boolean;
  authorName: string | null;
};

export type SteamGridDbGameIdentity = {
  localGameId: string;
  title: string;
  steamAppId: number | null;
  steamgriddbGameId: number | null;
  source: string;
  status: string;
};

export type ArtworkSearchResult = {
  queryId: string;
  gameId: string;
  slot: ArtworkSlot;
  styleFilter: ArtworkFilterKind;
  identity: SteamGridDbGameIdentity;
  candidates: ArtworkPreviewCandidate[];
};

export type ArtworkApplyRequest = {
  gameId: string;
  slot: ArtworkSlot;
  styleFilter: ArtworkFilterKind;
  candidateId: string;
};

export type ArtworkApplyResult = {
  gameId: string;
  slot: ArtworkSlot;
  cachedPath: string;
  cacheKey: string;
  checksum: string;
  width: number;
  height: number;
  cachedMimeType: string;
  fileReused: boolean;
};

export const ARTWORK_SLOTS: readonly ArtworkSlot[] = [
  "grid_horizontal",
  "grid_vertical",
  "grid_square",
  "hero",
  "logo",
  "icon",
];

export const ARTWORK_FILTERS: readonly ArtworkFilterKind[] = [
  "all",
  "no_logo",
  "other",
];

export function artworkSlotLabel(slot: ArtworkSlot): string {
  switch (slot) {
    case "grid_horizontal":
      return "Horizontal";
    case "grid_vertical":
      return "Vertical";
    case "grid_square":
      return "Square";
    case "hero":
      return "Hero";
    case "logo":
      return "Logo";
    case "icon":
      return "Icon";
  }
}

export function artworkFilterLabel(filter: ArtworkFilterKind): string {
  switch (filter) {
    case "all":
      return "Todos";
    case "no_logo":
      return "Sin logo";
    case "other":
      return "Otros estilos";
  }
}
