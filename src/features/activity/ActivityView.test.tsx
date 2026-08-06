import { act } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRoot, type Root } from "react-dom/client";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { Game } from "../catalog/game-types";
import type { ActivitySnapshot } from "./activity-types";
import { FocusScope } from "../../ui/navigation/focus/FocusScope";
import { NavigationProvider } from "../../ui/navigation/NavigationProvider";
import { ActivityView } from "./ActivityView";

const activityMocks = vi.hoisted(() => ({
  get: vi.fn(),
  getFriends: vi.fn(),
}));

vi.mock("./activity-service", () => ({
  activityService: {
    get: activityMocks.get,
    getFriends: activityMocks.getFriends,
  },
  activityErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Activity request failed",
}));

const snapshot: ActivitySnapshot = {
  status: "ready",
  metrics: null,
  lastSession: null,
  sessions: [],
  events: [],
  stats: [],
  streak: { currentDays: 3, bestDays: 7 },
  friends: [],
  friendsStatus: "no-data",
  sources: [],
};

function makeGame(id: string): Game {
  return {
    id,
    title: `Game ${id}`,
    sortTitle: `Game ${id}`,
    platform: "Windows",
    provider: "local",
    coverUrl: "",
    verticalCoverUrl: "",
    logoUrl: "",
    backgroundUrl: "",
    screenshots: [],
    description: "",
    genres: [],
    releaseYear: 2024,
    playtimeMinutes: 0,
    lastPlayedAt: null,
    favorite: false,
    installed: true,
    progress: 0,
    status: "not-started",
  };
}

function renderActivity(game = makeGame("game-001")): {
  host: HTMLDivElement;
  root: Root;
  queryClient: QueryClient;
} {
  const host = document.createElement("div");
  document.body.appendChild(host);
  const root = createRoot(host);
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  act(() => {
    root.render(
      <QueryClientProvider client={queryClient}>
        <NavigationProvider>
          <FocusScope scopeId="details" activateOnMount>
            <ActivityView game={game} />
          </FocusScope>
        </NavigationProvider>
      </QueryClientProvider>,
    );
  });
  return { host, root, queryClient };
}

async function flushEffects(): Promise<void> {
  await act(async () => {
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
  });
}

function cleanup({ host, root }: { host: HTMLDivElement; root: Root }): void {
  act(() => root.unmount());
  host.remove();
}

describe("ActivityView", () => {
  beforeEach(() => {
    activityMocks.get.mockReset();
    activityMocks.getFriends.mockReset().mockResolvedValue([]);
  });

  it("loads automatically and renders the grid without the redundant header", async () => {
    activityMocks.get.mockResolvedValue(snapshot);
    const rendered = renderActivity();

    await flushEffects();

    expect(activityMocks.get).toHaveBeenCalledWith("game-001");
    expect(rendered.host.querySelector(".activity-layout")).not.toBeNull();
    expect(rendered.host.textContent).toContain("Últimos 7 días");
    expect(
      rendered.host.querySelectorAll(".activity-weekdays span"),
    ).toHaveLength(7);
    expect(
      rendered.host.querySelector('[data-focus-id="details-activity-refresh"]'),
    ).toBeNull();
    expect(rendered.host.textContent).not.toContain("Actividad");
    expect(rendered.host.textContent).not.toContain("Tu historial de juego");
    expect(rendered.host.textContent).not.toContain(
      "Sesiones locales, logros y datos sociales disponibles.",
    );
    cleanup(rendered);
  });

  it("limits the timeline to six events", async () => {
    activityMocks.get.mockResolvedValue({
      ...snapshot,
      events: Array.from({ length: 7 }, (_, index) => ({
        id: `event-${index}`,
        eventType: "session_completed",
        occurredAt: String(1_700_000_000 - index * 60),
        title: `Evento ${index}`,
        description: null,
        value: null,
        source: "local",
      })),
    });
    const rendered = renderActivity();

    await flushEffects();

    expect(
      rendered.host.querySelectorAll(".activity-timeline-event"),
    ).toHaveLength(6);
    cleanup(rendered);
  });

  it("queries the new game when the game prop changes", async () => {
    activityMocks.get.mockResolvedValue(snapshot);
    const rendered = renderActivity();
    await flushEffects();

    act(() => {
      rendered.root.render(
        <QueryClientProvider client={rendered.queryClient}>
          <NavigationProvider>
            <FocusScope scopeId="details" activateOnMount>
              <ActivityView game={makeGame("game-002")} />
            </FocusScope>
          </NavigationProvider>
        </QueryClientProvider>,
      );
    });
    await flushEffects();

    expect(activityMocks.get).toHaveBeenNthCalledWith(2, "game-002");
    cleanup(rendered);
  });

  it("keeps the initial loading panel free of navigable controls", async () => {
    let resolveActivity: (value: ActivitySnapshot) => void = () => undefined;
    activityMocks.get.mockReturnValue(
      new Promise<ActivitySnapshot>((resolve) => {
        resolveActivity = resolve;
      }),
    );
    const rendered = renderActivity();

    expect(rendered.host.querySelector(".activity-state-panel")).not.toBeNull();
    expect(rendered.host.querySelector('[data-focusable="true"]')).toBeNull();

    await act(async () => resolveActivity(snapshot));
    cleanup(rendered);
  });

  it("keeps retry available for an initial error", async () => {
    activityMocks.get.mockRejectedValue(new Error("Initial activity error"));
    const rendered = renderActivity();

    await flushEffects();

    expect(rendered.host.textContent).toContain("Reintentar");
    expect(
      rendered.host.querySelector('[data-focus-id="details-activity-retry"]'),
    ).not.toBeNull();
    expect(
      rendered.host.querySelector('[data-focus-id="details-activity-refresh"]'),
    ).toBeNull();
    cleanup(rendered);
  });

  it("keeps the previous cards when a later request fails", async () => {
    activityMocks.get
      .mockResolvedValueOnce(snapshot)
      .mockRejectedValueOnce(new Error("Later activity error"));
    const rendered = renderActivity();
    await flushEffects();

    act(() => {
      rendered.root.render(
        <QueryClientProvider client={rendered.queryClient}>
          <NavigationProvider>
            <FocusScope scopeId="details" activateOnMount>
              <ActivityView game={makeGame("game-002")} />
            </FocusScope>
          </NavigationProvider>
        </QueryClientProvider>,
      );
    });
    await flushEffects();

    expect(rendered.host.querySelector(".activity-layout")).not.toBeNull();
    expect(rendered.host.textContent).toContain("Later activity error");
    cleanup(rendered);
  });
});
