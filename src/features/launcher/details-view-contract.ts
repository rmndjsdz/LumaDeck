export const DETAILS_TAB_ORDER = [
  "summary",
  "performance",
  "activity",
  "achievements",
  "news",
  "dlc",
  "related",
  "reviews",
] as const;

export type DetailsSection = (typeof DETAILS_TAB_ORDER)[number];

export const DETAILS_ATOMIC_CONTENT = {
  summary: ["description", "features", "screenshots"],
  performance: ["capabilities", "recommended-profile"],
} as const;
