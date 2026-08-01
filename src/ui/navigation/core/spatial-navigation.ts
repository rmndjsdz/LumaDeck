import type {
  NavigationDirection,
  Rect,
  SpatialCandidate,
  SpatialResolution,
} from "./navigation-types";

interface ScoredCandidate {
  candidate: SpatialCandidate;
  primary: number;
  perpendicular: number;
  overlap: number;
  score: number;
}

function center(rect: Rect): { x: number; y: number } {
  return {
    x: rect.left + rect.width / 2,
    y: rect.top + rect.height / 2,
  };
}

function axisOverlap(
  aStart: number,
  aEnd: number,
  bStart: number,
  bEnd: number,
) {
  return Math.max(0, Math.min(aEnd, bEnd) - Math.max(aStart, bStart));
}

function scoreCandidate(
  current: Rect,
  candidate: SpatialCandidate,
  direction: NavigationDirection,
): ScoredCandidate | null {
  const currentCenter = center(current);
  const candidateCenter = center(candidate.rect);
  let primary: number;
  let perpendicular: number;
  let overlap: number;

  if (direction === "up" || direction === "down") {
    const isForward =
      direction === "up"
        ? candidateCenter.y < currentCenter.y
        : candidateCenter.y > currentCenter.y;
    if (!isForward) return null;
    primary = Math.abs(candidateCenter.y - currentCenter.y);
    perpendicular = Math.abs(candidateCenter.x - currentCenter.x);
    overlap = axisOverlap(
      current.left,
      current.right,
      candidate.rect.left,
      candidate.rect.right,
    );
  } else {
    const isForward =
      direction === "left"
        ? candidateCenter.x < currentCenter.x
        : candidateCenter.x > currentCenter.x;
    if (!isForward) return null;
    primary = Math.abs(candidateCenter.x - currentCenter.x);
    perpendicular = Math.abs(candidateCenter.y - currentCenter.y);
    overlap = axisOverlap(
      current.top,
      current.bottom,
      candidate.rect.top,
      candidate.rect.bottom,
    );
  }

  const alignmentPenalty = overlap === 0 ? 24 : 0;
  const priorityBonus = (candidate.priority ?? 0) * 0.01;
  return {
    candidate,
    primary,
    perpendicular,
    overlap,
    score: primary + perpendicular * 0.35 + alignmentPenalty - priorityBonus,
  };
}

export function findSpatialCandidate(
  current: Rect,
  candidates: readonly SpatialCandidate[],
  direction: NavigationDirection,
): SpatialResolution {
  const startedAt = performance.now();
  const evaluated: ScoredCandidate[] = [];

  for (const candidate of candidates) {
    if (
      candidate.disabled ||
      candidate.hidden ||
      candidate.connected === false
    ) {
      continue;
    }
    const scored = scoreCandidate(current, candidate, direction);
    if (scored) evaluated.push(scored);
  }

  evaluated.sort((a, b) => {
    if (a.score !== b.score) return a.score - b.score;
    if (a.primary !== b.primary) return a.primary - b.primary;
    if (a.perpendicular !== b.perpendicular) {
      return a.perpendicular - b.perpendicular;
    }
    return a.candidate.focusId.localeCompare(b.candidate.focusId);
  });

  return {
    candidate: evaluated[0]?.candidate ?? null,
    evaluated: evaluated.map(({ candidate }) => candidate.focusId),
    durationMs: performance.now() - startedAt,
  };
}
