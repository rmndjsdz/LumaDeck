import pegi12 from "../../../assets/age-rating/PEGI-12.png";
import pegi16 from "../../../assets/age-rating/PEGI-16.png";
import pegi18 from "../../../assets/age-rating/PEGI-18.png";
import pegi3 from "../../../assets/age-rating/PEGI-3.png";
import pegi7 from "../../../assets/age-rating/PEGI-7.png";

export type AgeRating = "3" | "7" | "12" | "16" | "18";

const AGE_RATING_ASSETS: Readonly<Record<AgeRating, string>> = {
  "3": pegi3,
  "7": pegi7,
  "12": pegi12,
  "16": pegi16,
  "18": pegi18,
};

export function getAgeRatingAsset(
  value: string | number | null | undefined,
): string | undefined {
  if (value === null || value === undefined) return undefined;
  const normalized = String(value)
    .replace(/^PEGI\s*/i, "")
    .replace(/\+$/, "")
    .trim();
  const ageRating = Object.keys(AGE_RATING_ASSETS).find(
    (key): key is AgeRating => key === normalized,
  );
  return ageRating ? AGE_RATING_ASSETS[ageRating] : undefined;
}
