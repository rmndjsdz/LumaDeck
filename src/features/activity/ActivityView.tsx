import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useQueryClient } from "@tanstack/react-query";
import type { Game } from "../catalog/game-types";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { activityErrorMessage, activityService } from "./activity-service";
import { useActivity } from "./activity-query";
import type {
  ActivityEvent,
  ActivitySnapshot,
  ActivityStat,
} from "./activity-types";

export function ActivityView({ game }: { game: Game }) {
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [previousSnapshot, setPreviousSnapshot] =
    useState<ActivitySnapshot | null>(null);
  const queryClient = useQueryClient();
  const activityQuery = useActivity(game.id);
  const friendsRefreshGameRef = useRef<string | null>(null);

  useEffect(() => {
    if (activityQuery.data) {
      setPreviousSnapshot(activityQuery.data);
    }
  }, [activityQuery.data]);

  const loadActivity = useCallback(async () => {
    setErrorMessage(null);
    await activityQuery.refetch();
  }, [activityQuery]);

  useEffect(() => {
    if (activityQuery.isError && activityQuery.error) {
      setErrorMessage(activityErrorMessage(activityQuery.error));
    }
  }, [activityQuery.error, activityQuery.isError]);

  useEffect(() => {
    if (!activityQuery.data || friendsRefreshGameRef.current === game.id) {
      return;
    }
    friendsRefreshGameRef.current = game.id;
    void activityService
      .getFriends(game.id)
      .then((friends) => {
        queryClient.setQueryData<ActivitySnapshot>(
          ["activity", game.id],
          (current) =>
            current
              ? {
                  ...current,
                  friends,
                  friendsStatus: friends.length > 0 ? "ready" : "no-data",
                }
              : current,
        );
      })
      .catch((error: unknown) => {
        setErrorMessage(activityErrorMessage(error));
        queryClient.setQueryData<ActivitySnapshot>(
          ["activity", game.id],
          (current) =>
            current
              ? {
                  ...current,
                  friendsStatus:
                    error instanceof Error && error.message === "STEAM_OFFLINE"
                      ? "offline"
                      : "unavailable",
                }
              : current,
        );
      });
  }, [activityQuery.data, game.id, queryClient]);

  const snapshot = activityQuery.data ?? previousSnapshot;

  return (
    <section className="details-activity" aria-label="Game activity">
      {activityQuery.isError && !snapshot ? (
        <ActivityStatePanel
          title="No se pudo consultar la actividad"
          message={errorMessage ?? "Intenta nuevamente."}
          action={
            <Focusable
              focusId="details-activity-retry"
              scopeId="details"
              className="activity-state-action"
              onConfirm={() => void loadActivity()}
            >
              Reintentar
            </Focusable>
          }
        />
      ) : activityQuery.isPending && !snapshot ? (
        <ActivityStatePanel
          title="Consultando actividad"
          message="Preparando los datos disponibles para este juego."
        />
      ) : snapshot ? (
        <>
          {errorMessage && (
            <p className="activity-inline-warning" role="status">
              {errorMessage}
            </p>
          )}
          <div className="activity-layout">
            <div className="activity-column activity-column-left">
              <LastSessionCard game={game} snapshot={snapshot} />
              <StreakCard snapshot={snapshot} />
            </div>
            <TimelineCard snapshot={snapshot} />
            <div className="activity-column activity-column-right">
              <StatsCard snapshot={snapshot} />
              <FriendsCard snapshot={snapshot} />
            </div>
          </div>
        </>
      ) : null}
    </section>
  );
}

function LastSessionCard({
  game,
  snapshot,
}: {
  game: Game;
  snapshot: ActivitySnapshot;
}) {
  const session = snapshot.lastSession;
  return (
    <article className="activity-card activity-last-session">
      <p className="activity-card-title">Última sesión</p>
      {session ? (
        <div className="activity-session-summary">
          <img
            src={game.iconUrl || game.coverUrl}
            alt=""
            className="activity-game-icon"
            draggable={false}
          />
          <div className="activity-session-meta">
            <ActivityIcon name="clock" />
            <div>
              <span>{formatDateTime(session.startedAt)}</span>
              <strong>{formatSessionDuration(session.durationSeconds)}</strong>
            </div>
          </div>
          <img
            src={game.backgroundUrl}
            alt=""
            className="activity-session-art"
            draggable={false}
          />
        </div>
      ) : (
        <ActivityEmptyContent>
          Todavía no hay sesiones registradas por LumaDeck.
        </ActivityEmptyContent>
      )}
    </article>
  );
}

function StreakCard({ snapshot }: { snapshot: ActivitySnapshot }) {
  const activeWeekdays = getActiveWeekdays(
    snapshot.sessions,
    snapshot.streak.currentDays,
  );

  return (
    <article className="activity-card activity-streak-card">
      <p className="activity-card-title">Racha de juego</p>
      <div className="activity-streak-layout">
        <div className="activity-streak-summary">
          <div className="activity-streak-value">
            <ActivityIcon name="flame" />
            <strong>{snapshot.streak.currentDays} días</strong>
          </div>
          <p className="activity-muted-copy">
            Mejor racha: {snapshot.streak.bestDays} días
          </p>
        </div>
        <div className="activity-streak-days">
          <p className="activity-card-title">Últimos 7 días</p>
          <div className="activity-weekdays" aria-label="Días con sesiones">
            {WEEKDAYS.map((day, index) => (
              <span
                key={day}
                className={activeWeekdays.has(index) ? "is-active" : undefined}
                aria-hidden="true"
              >
                {day}
              </span>
            ))}
          </div>
        </div>
      </div>
    </article>
  );
}

function TimelineCard({ snapshot }: { snapshot: ActivitySnapshot }) {
  return (
    <article className="activity-card activity-timeline-card">
      <div className="activity-card-heading-row">
        <p className="activity-card-title">Línea de tiempo</p>
        <span className={`activity-data-state is-${snapshot.status}`}>
          {activityStatusLabel(snapshot.status)}
        </span>
      </div>
      {snapshot.events.length > 0 ? (
        <div className="activity-timeline">
          {snapshot.events.slice(0, 6).map((event) => (
            <TimelineEvent key={event.id} event={event} />
          ))}
        </div>
      ) : (
        <ActivityEmptyContent>
          Los eventos aparecerán cuando haya sesiones o logros sincronizados.
        </ActivityEmptyContent>
      )}
    </article>
  );
}

function TimelineEvent({ event }: { event: ActivityEvent }) {
  return (
    <div className="activity-timeline-event">
      <div className="activity-timeline-marker">
        <ActivityIcon name={eventIcon(event.eventType)} />
      </div>
      <div className="activity-timeline-copy">
        <span>{formatDateTime(event.occurredAt)}</span>
        <strong>{event.title}</strong>
        {event.description && <p>{event.description}</p>}
      </div>
      <span className="activity-event-source">{sourceLabel(event.source)}</span>
    </div>
  );
}

function StatsCard({ snapshot }: { snapshot: ActivitySnapshot }) {
  const metrics = snapshot.metrics;
  return (
    <article className="activity-card activity-stats-card">
      <p className="activity-card-title">Estadísticas de juego</p>
      {metrics ? (
        <dl className="activity-stats-list">
          <StatRow
            label="Tiempo jugado"
            value={formatMinutes(metrics.totalPlaytimeMinutes)}
          />
          <StatRow
            label="Logros desbloqueados"
            value={`${metrics.achievementUnlocked ?? "—"} / ${metrics.achievementTotal ?? "—"}`}
          />
          <StatRow
            label="Progreso"
            value={`${Math.round(metrics.progress)}%`}
          />
          {snapshot.stats.slice(0, 3).map((stat) => (
            <StatRow
              key={stat.key}
              label={stat.label}
              value={formatValue(stat)}
            />
          ))}
        </dl>
      ) : (
        <ActivityEmptyContent>
          Steam todavía no tiene métricas disponibles para este juego.
        </ActivityEmptyContent>
      )}
    </article>
  );
}

function FriendsCard({ snapshot }: { snapshot: ActivitySnapshot }) {
  return (
    <article className="activity-card activity-friends-card">
      <div className="activity-card-heading-row">
        <p className="activity-card-title">Amigos que juegan</p>
        <span className={`activity-data-state is-${snapshot.friendsStatus}`}>
          {activityStatusLabel(snapshot.friendsStatus)}
        </span>
      </div>
      {snapshot.friends.length > 0 ? (
        <div className="activity-friends-list">
          {snapshot.friends.slice(0, 4).map((friend) => (
            <div className="activity-friend" key={friend.steamId64}>
              <img
                src={friend.avatarUrl}
                alt=""
                className="activity-friend-avatar"
                draggable={false}
              />
              <div>
                <strong>{friend.personaName}</strong>
                <span>{friend.gameName ?? "Jugando ahora"}</span>
              </div>
              <span className="activity-friend-state">
                {friend.personaState}
              </span>
            </div>
          ))}
        </div>
      ) : (
        <ActivityEmptyContent>
          {snapshot.friendsStatus === "offline"
            ? "Steam está desconectado."
            : snapshot.friendsStatus === "unavailable"
              ? "La lista de amigos no está disponible."
              : snapshot.friendsStatus === "pending"
                ? "Consultando amigos en Steam..."
                : "Ningún amigo está jugando este título ahora."}
        </ActivityEmptyContent>
      )}
    </article>
  );
}

function StatRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="activity-stat-row">
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function ActivityStatePanel({
  title,
  message,
  action,
}: {
  title: string;
  message: string;
  action?: ReactNode;
}) {
  return (
    <div className="activity-state-panel">
      <strong>{title}</strong>
      <p>{message}</p>
      {action}
    </div>
  );
}

function ActivityEmptyContent({ children }: { children: ReactNode }) {
  return <p className="activity-empty-copy">{children}</p>;
}

function ActivityIcon({ name }: { name: string }) {
  return (
    <span className={`activity-icon activity-icon-${name}`} aria-hidden="true">
      {name === "clock"
        ? "◷"
        : name === "trophy"
          ? "♜"
          : name === "flame"
            ? "♨"
            : "⌁"}
    </span>
  );
}

function eventIcon(eventType: string): string {
  if (eventType === "achievement_unlocked") return "trophy";
  if (eventType.startsWith("session_")) return "clock";
  return "event";
}

function formatMinutes(value: number): string {
  const hours = Math.floor(value / 60);
  const minutes = value % 60;
  return hours > 0 ? `${hours} h ${minutes} min` : `${minutes} min`;
}

function formatSessionDuration(value: number | null): string {
  if (value === null) return "Duración no disponible";
  return formatMinutes(Math.floor(value / 60));
}

function formatDateTime(value: string): string {
  const timestamp = Number(value);
  if (!Number.isFinite(timestamp) || timestamp <= 0)
    return "Fecha no disponible";
  return new Intl.DateTimeFormat("es-SV", {
    day: "2-digit",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(timestamp * 1000));
}

function formatValue(stat: ActivityStat): string {
  if (typeof stat.value === "string" || typeof stat.value === "number") {
    return String(stat.value);
  }
  if (typeof stat.value === "boolean") return stat.value ? "Sí" : "No";
  return "Disponible";
}

function activityStatusLabel(status: string): string {
  if (status === "ready") return "Disponible";
  if (status === "no-data") return "Sin datos";
  if (status === "offline") return "Sin conexión";
  if (status === "pending") return "Consultando";
  return "No disponible";
}

function sourceLabel(source: string): string {
  return source === "steam" ? "Steam" : "LumaDeck";
}

function getActiveWeekdays(
  sessions: ActivitySnapshot["sessions"],
  currentDays: number,
): Set<number> {
  const activeWeekdays = new Set<number>();
  const sessionTimestamps = sessions
    .map((session) => Number(session.startedAt) * 1000)
    .filter((timestamp) => Number.isFinite(timestamp));
  const latestTimestamp = Math.max(...sessionTimestamps);
  const daysToMark = Math.min(Math.max(Math.floor(currentDays), 0), 7);

  if (!Number.isFinite(latestTimestamp) || daysToMark === 0) {
    return activeWeekdays;
  }

  const latestWeekday = new Date(latestTimestamp).getDay();
  for (let offset = 0; offset < daysToMark; offset += 1) {
    const weekday = (latestWeekday - offset + 7) % 7;
    activeWeekdays.add(weekday === 0 ? 6 : weekday - 1);
  }

  return activeWeekdays;
}

const WEEKDAYS = ["L", "M", "X", "J", "V", "S", "D"];
