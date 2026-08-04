import { useCallback, useEffect, useState, type ReactNode } from "react";
import type { Game } from "../catalog/game-types";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { activityErrorMessage, activityService } from "./activity-service";
import type {
  ActivityEvent,
  ActivitySnapshot,
  ActivityStat,
} from "./activity-types";

export function ActivityView({ game }: { game: Game }) {
  const [snapshot, setSnapshot] = useState<ActivitySnapshot | null>(null);
  const [state, setState] = useState<"loading" | "ready" | "error">("loading");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const loadActivity = useCallback(async () => {
    setState("loading");
    setErrorMessage(null);
    try {
      const nextSnapshot = await activityService.get(game.id);
      setSnapshot(nextSnapshot);
      setState("ready");
    } catch (error) {
      setState("error");
      setErrorMessage(activityErrorMessage(error));
    }
  }, [game.id]);

  useEffect(() => {
    void loadActivity();
  }, [loadActivity]);

  return (
    <section className="details-activity" aria-labelledby="activity-heading">
      <div className="activity-heading">
        <div>
          <p className="eyebrow">Actividad</p>
          <h2 id="activity-heading">Tu historial de juego</h2>
          <p className="activity-heading-copy">
            Sesiones locales, logros y datos sociales disponibles.
          </p>
        </div>
        <Focusable
          focusId="details-activity-refresh"
          scopeId="details"
          className="activity-refresh-button"
          disabled={state === "loading"}
          onConfirm={() => void loadActivity()}
        >
          {state === "loading" ? "Consultando…" : "Actualizar"}
        </Focusable>
      </div>

      {state === "error" && !snapshot ? (
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
      ) : state === "loading" && !snapshot ? (
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
          <div className="activity-session-meta">
            <ActivityIcon name="clock" />
            <div>
              <span>{formatDateTime(session.startedAt)}</span>
              <strong>{formatSessionDuration(session.durationSeconds)}</strong>
            </div>
          </div>
          <p className="activity-session-status">
            {session.status === "active"
              ? "Sesión en curso"
              : session.status === "interrupted"
                ? "Sesión interrumpida"
                : "Sesión completada"}
          </p>
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
  return (
    <article className="activity-card activity-streak-card">
      <p className="activity-card-title">Racha de juego</p>
      <div className="activity-streak-value">
        <ActivityIcon name="flame" />
        <strong>{snapshot.streak.currentDays} días</strong>
      </div>
      <p className="activity-muted-copy">
        Mejor racha: {snapshot.streak.bestDays} días
      </p>
      <div className="activity-weekdays" aria-label="Días con sesiones">
        {WEEKDAYS.map((day) => (
          <span key={day} aria-hidden="true">
            {day}
          </span>
        ))}
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
          {snapshot.events.slice(0, 8).map((event) => (
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

const WEEKDAYS = ["L", "M", "X", "J", "V", "S", "D"];
