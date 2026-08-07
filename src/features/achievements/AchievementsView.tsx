import { useMemo, type CSSProperties } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type {
  Achievement,
  AchievementDistributions,
  AchievementSummary,
} from "./achievement-types";
import {
  useAchievementDistributions,
  useAchievements,
  useAchievementSummary,
  useAutoRefreshGameAchievements,
} from "./achievement-query";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { NavigationGrid } from "../../ui/navigation/layouts/NavigationGrid";

const ACHIEVEMENT_ASSET_PATH = "/themes/cinematic/assets";

const VIRTUAL_TIER_LABELS = {
  bronze: "Bronce",
  silver: "Plata",
  gold: "Oro",
} as const;

const RARITY_LABELS = {
  "very-rare": "Muy raro",
  rare: "Raro",
  uncommon: "Poco común",
  common: "Común",
  "very-common": "Muy común",
} as const;

const RARITY_DISTRIBUTION = [
  {
    id: "very-rare",
    label: "Muy raro",
    range: "< 5%",
    rarityIds: ["very-rare"],
  },
  { id: "rare", label: "Raro", range: "5% – 20%", rarityIds: ["rare"] },
  {
    id: "uncommon",
    label: "Poco común",
    range: "20% – 50%",
    rarityIds: ["uncommon"],
  },
  {
    id: "common",
    label: "Común",
    range: "≥ 50%",
    rarityIds: ["common", "very-common"],
  },
] as const;

export function AchievementsView({ gameId }: { gameId: string }) {
  const achievementsQuery = useAchievements(gameId);
  const summaryQuery = useAchievementSummary(gameId);
  const distributionsQuery = useAchievementDistributions(gameId);
  useAutoRefreshGameAchievements(gameId);

  const achievements = useMemo(
    () =>
      [...(achievementsQuery.data?.achievements ?? [])].sort(
        (left, right) => Number(right.unlocked) - Number(left.unlocked),
      ),
    [achievementsQuery.data?.achievements],
  );
  const summary = summaryQuery.data;
  const distributions = distributionsQuery.data;
  const recent = useMemo(
    () =>
      [...achievements]
        .filter((achievement) => achievement.unlocked)
        .sort(
          (left, right) =>
            Number(right.unlockTime ?? 0) - Number(left.unlockTime ?? 0),
        )
        .slice(0, 4),
    [achievements],
  );

  if (achievementsQuery.isPending || summaryQuery.isPending) {
    return <p className="achievement-state">Cargando logros…</p>;
  }
  if (achievementsQuery.isError || summaryQuery.isError) {
    return (
      <p className="achievement-state">No se pudieron cargar los logros.</p>
    );
  }

  return (
    <section
      className="details-achievements"
      aria-labelledby="details-achievements-heading"
    >
      <div className="achievements-main-column">
        <AchievementsHeader summary={summary} distributions={distributions} />
        {achievements.length > 0 ? (
          <div
            className="achievements-list-scroll"
            data-scroll-scope="achievements"
          >
            <NavigationGrid
              groupId="achievements-list"
              columns={1}
              itemCount={achievements.length}
              resolveFocusId={(index) =>
                `achievement-${achievements[index]?.apiName ?? ""}`
              }
              className="achievements-list"
            >
              {achievements.map((achievement, index) => (
                <AchievementRow
                  key={achievement.apiName}
                  achievement={achievement}
                  focusId={`achievement-${achievement.apiName}`}
                  gridIndex={index}
                  isFirst={index === 0}
                />
              ))}
            </NavigationGrid>
          </div>
        ) : (
          <p className="achievement-state">Este juego aún no tiene logros.</p>
        )}
      </div>
      <AchievementsAside
        achievements={achievements}
        summary={summary}
        recent={recent}
      />
    </section>
  );
}

export function AchievementsHeader({
  summary,
  distributions,
}: {
  summary: AchievementSummary | undefined;
  distributions: AchievementDistributions | undefined;
}) {
  const unlockedDistribution = distributions?.unlocked;
  const trophyCounts = [
    {
      asset: "platinum.png",
      label: "Platinum",
      value: 0,
    },
    {
      asset: "oro.png",
      label: "Oro",
      value: unlockedDistribution?.gold ?? 0,
    },
    {
      asset: "plata.png",
      label: "Plata",
      value: unlockedDistribution?.silver ?? 0,
    },
    {
      asset: "bronce.png",
      label: "Bronce",
      value: unlockedDistribution?.bronze ?? 0,
    },
  ] as const;

  return (
    <header className="achievements-header">
      <img
        className="achievements-header-trophy"
        src={`${ACHIEVEMENT_ASSET_PATH}/plata.png`}
        alt=""
        draggable={false}
      />
      <div className="achievements-header-count-copy">
        <p className="eyebrow">Progreso de logros</p>
        <h2 id="details-achievements-heading">
          {summary?.unlocked ?? 0} / {summary?.total ?? 0}
        </h2>
      </div>
      <strong className="achievements-header-percentage">
        {formatPercentage(summary?.completionPercentage)}
      </strong>
      <div className="achievements-header-bar">
        <div className="achievement-progress-track" aria-hidden="true">
          <span
            className="achievement-progress-fill"
            style={{ width: `${summary?.completionPercentage ?? 0}%` }}
          />
        </div>
      </div>
      <div className="achievements-header-trophy-counts" aria-label="Trofeos">
        {trophyCounts.map((trophy) => (
          <span className="achievements-header-trophy-count" key={trophy.label}>
            <img
              src={`${ACHIEVEMENT_ASSET_PATH}/${trophy.asset}`}
              alt=""
              draggable={false}
            />
            <strong>{trophy.value}</strong>
            <span className="visually-hidden">{trophy.label}</span>
          </span>
        ))}
      </div>
      <div className="achievements-sort-control">
        <span>Ordenar por</span>
        <select
          className="achievements-sort-select"
          aria-label="Ordenar logros"
          defaultValue="default"
          disabled
        >
          <option value="default">Predeterminado</option>
        </select>
      </div>
    </header>
  );
}

function AchievementRow({
  achievement,
  focusId,
  gridIndex,
  isFirst,
}: {
  achievement: Achievement;
  focusId: string;
  gridIndex: number;
  isFirst: boolean;
}) {
  const icon = getAchievementIcon(achievement);
  return (
    <Focusable
      focusId={focusId}
      scopeId="details"
      gridIndex={gridIndex}
      className={`achievement-row${achievement.unlocked ? "" : " is-locked"}`}
      navigation={isFirst ? { up: "details-tab-achievements" } : undefined}
      ariaLabel={`${achievement.displayName}, ${RARITY_LABELS[achievement.rarity]}, ${
        achievement.unlocked ? "desbloqueado" : "bloqueado"
      }`}
    >
      <AchievementIcon src={icon} />
      <span className="achievement-row-copy">
        <span className="achievement-row-heading">
          <strong>{achievement.displayName}</strong>
          <span className={`achievement-rarity is-${achievement.rarity}`}>
            {RARITY_LABELS[achievement.rarity]}
          </span>
        </span>
        <span className="achievement-description">
          {achievement.description}
        </span>
        <span className="achievement-row-meta">
          {achievement.unlocked
            ? `Desbloqueado ${formatUnlockDate(achievement.unlockTime)}`
            : "Bloqueado"}
          {achievement.unlockPercentage !== null && (
            <span>{formatPercentage(achievement.unlockPercentage)} global</span>
          )}
        </span>
      </span>
      <span
        className="achievement-row-tier"
        aria-label={`Trofeo ${VIRTUAL_TIER_LABELS[achievement.virtualTier]}`}
      >
        <img
          src={`${ACHIEVEMENT_ASSET_PATH}/${getTierAsset(achievement.virtualTier)}`}
          alt=""
          draggable={false}
        />
      </span>
      <span className="achievement-row-state">
        {achievement.unlocked ? "Desbloqueado" : "Bloqueado"}
      </span>
    </Focusable>
  );
}

function AchievementIcon({ src }: { src: string | null }) {
  return src ? (
    <img
      className="achievement-icon"
      src={resolveCachedAssetUrl(src)}
      alt=""
      draggable={false}
    />
  ) : (
    <span
      className="achievement-icon achievement-icon-placeholder"
      aria-hidden="true"
    />
  );
}

function getAchievementIcon(achievement: Achievement): string | null {
  if (achievement.unlocked) {
    return achievement.localIconUnlocked ?? achievement.iconUnlocked;
  }
  return achievement.localIconLocked ?? achievement.iconLocked;
}

function AchievementsAside({
  achievements,
  summary,
  recent,
}: {
  achievements: Achievement[];
  summary: AchievementSummary | undefined;
  recent: Achievement[];
}) {
  return (
    <aside className="achievements-aside" aria-label="Estadísticas de logros">
      <section className="achievement-panel achievement-summary-panel">
        <p className="achievement-summary-heading">Resumen de logros</p>
        <div
          className="achievement-ring"
          style={
            {
              "--achievement-ring-progress": `${summary?.completionPercentage ?? 0}%`,
            } as CSSProperties
          }
        >
          <div className="achievement-ring-inner">
            <strong>{formatPercentage(summary?.completionPercentage)}</strong>
            <span>Completado</span>
          </div>
        </div>
        <div className="achievement-summary-legend">
          <span>
            <i className="is-complete" />
            Completados
          </span>
          <span>
            <i className="is-progress" />
            En progreso
          </span>
          <span>
            <i className="is-start" />
            Sin comenzar
          </span>
        </div>
        <div className="achievement-summary-values" aria-label="Valores">
          <strong>{summary?.unlocked ?? "—"}</strong>
          <strong>—</strong>
          <strong>{summary?.locked ?? "—"}</strong>
        </div>
      </section>
      <DistributionPanel achievements={achievements} />
      <section className="achievement-panel achievement-recent-panel">
        <div className="achievement-panel-heading">
          <div>
            <h3>ÚLTIMOS LOGROS</h3>
          </div>
          <span className="achievement-view-all">Ver todos</span>
        </div>
        <div className="achievement-recent-list">
          {recent.map((achievement) => (
            <div className="achievement-recent-item" key={achievement.apiName}>
              <AchievementIcon src={getAchievementIcon(achievement)} />
              <span>
                <strong>{achievement.displayName}</strong>
                <small>{formatUnlockDate(achievement.unlockTime)}</small>
              </span>
            </div>
          ))}
          {recent.length === 0 && (
            <p className="achievement-empty-copy">
              Aún no hay logros recientes.
            </p>
          )}
        </div>
      </section>
    </aside>
  );
}

export function DistributionPanel({
  achievements,
}: {
  achievements: Achievement[];
}) {
  const unlockedAchievements = achievements.filter(
    (achievement) => achievement.unlocked,
  );
  const total = unlockedAchievements.length;
  const values = RARITY_DISTRIBUTION.map((rarity) => {
    const count = unlockedAchievements.filter((achievement) =>
      rarity.rarityIds.some((rarityId) => rarityId === achievement.rarity),
    ).length;
    return {
      ...rarity,
      count,
      percentage: total > 0 ? Math.round((count / total) * 100) : 0,
    };
  });
  return (
    <section className="achievement-panel">
      <p className="achievement-distribution-heading">
        Distribución por rareza · desbloqueados
      </p>
      <div className="achievement-distribution-list">
        {values.map(({ id, label, range, count, percentage }) => (
          <div className={`achievement-distribution-row is-${id}`} key={id}>
            <span className="achievement-distribution-label">
              <i className="achievement-distribution-dot" aria-hidden="true" />
              {label} ({range})
            </span>
            <div className="achievement-distribution-track" aria-hidden="true">
              <span style={{ flexGrow: count }} />
            </div>
            <strong>{count}</strong>
            <strong>{percentage}%</strong>
          </div>
        ))}
      </div>
    </section>
  );
}

function resolveCachedAssetUrl(value: string): string {
  if (/^(https?:|data:|blob:|asset:|http:\/\/asset\.localhost)/i.test(value)) {
    return value;
  }
  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
    return convertFileSrc(value);
  }
  return value;
}

function getTierAsset(tier: Achievement["virtualTier"]): string {
  switch (tier) {
    case "gold":
      return "oro.png";
    case "silver":
      return "plata.png";
    case "bronze":
      return "bronce.png";
  }
}

function formatPercentage(value: number | null | undefined): string {
  return value === null || value === undefined ? "—" : `${value.toFixed(1)}%`;
}

function formatUnlockDate(value: string | null): string {
  if (!value) return "sin fecha";
  const timestamp = Number(value);
  const date = Number.isFinite(timestamp)
    ? new Date(timestamp > 10_000_000_000 ? timestamp : timestamp * 1000)
    : new Date(value);
  if (Number.isNaN(date.getTime())) return "sin fecha";
  return new Intl.DateTimeFormat(undefined, {
    day: "2-digit",
    month: "short",
    year: "numeric",
  }).format(date);
}
