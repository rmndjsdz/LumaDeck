import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

const WEATHER_REFRESH_MS = 15 * 60 * 1000;
const WEATHER_RETRY_DELAYS_MS = [1_000, 3_000, 10_000] as const;
const WEATHER_MAX_ATTEMPTS = WEATHER_RETRY_DELAYS_MS.length + 1;

type WeatherDay = {
  date: string;
  weatherCode: number;
  maxTemperature: number;
  minTemperature: number;
};

type WeatherData = {
  temperature: number;
  weatherCode: number;
  forecast: WeatherDay[];
};

type WeatherCoordinates = {
  latitude: number;
  longitude: number;
  source: "browser" | "ip";
};

type WeatherState =
  | { status: "loading"; data: null }
  | { status: "ready"; data: WeatherData }
  | { status: "unavailable"; data: null };

export function WeatherWidget() {
  const [weather, setWeather] = useState<WeatherState>({
    status: "loading",
    data: null,
  });
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    let disposed = false;
    let inFlight = false;
    let retryTimer: number | null = null;
    let resolveRetry: ((shouldContinue: boolean) => void) | null = null;

    const waitForRetry = (delayMs: number): Promise<boolean> =>
      new Promise((resolve) => {
        resolveRetry = resolve;
        retryTimer = window.setTimeout(() => {
          retryTimer = null;
          resolveRetry = null;
          resolve(true);
        }, delayMs);
      });

    const loadWeather = async () => {
      if (inFlight) {
        logWeatherEvent("load-skipped", { reason: "already-in-flight" });
        return;
      }

      inFlight = true;
      let lastError: unknown = new Error("Weather load failed");

      try {
        for (let attempt = 1; attempt <= WEATHER_MAX_ATTEMPTS; attempt += 1) {
          if (disposed) return;
          if (attempt > 1) {
            logWeatherEvent("retry-start", {
              attempt,
              maxAttempts: WEATHER_MAX_ATTEMPTS,
            });
          }
          logWeatherEvent("load-start", {
            attempt,
            maxAttempts: WEATHER_MAX_ATTEMPTS,
          });

          try {
            const position = await getCurrentPosition();
            logWeatherEvent("geolocation-ready", {
              latitude: roundCoordinate(position.latitude),
              longitude: roundCoordinate(position.longitude),
              source: position.source,
            });
            const url = new URL("https://api.open-meteo.com/v1/forecast");
            url.searchParams.set("latitude", String(position.latitude));
            url.searchParams.set("longitude", String(position.longitude));
            url.searchParams.set("current", "temperature_2m,weather_code");
            url.searchParams.set(
              "daily",
              "weather_code,temperature_2m_max,temperature_2m_min",
            );
            url.searchParams.set("forecast_days", "4");
            url.searchParams.set("temperature_unit", "celsius");
            url.searchParams.set("timezone", "auto");

            const response = await fetch(url, { cache: "no-store" });
            logWeatherEvent("request-response", {
              status: response.status,
              ok: response.ok,
            });
            if (!response.ok)
              throw new Error(`Weather request failed (${response.status})`);
            const payload: unknown = await response.json();
            const data = parseWeatherResponse(payload);
            logWeatherEvent("load-ready", {
              attempt,
              temperature: data.temperature,
              weatherCode: data.weatherCode,
              forecastDays: data.forecast.length,
            });
            if (!disposed) setWeather({ status: "ready", data });
            return;
          } catch (error) {
            lastError = error;
            const retryDelayMs = WEATHER_RETRY_DELAYS_MS[attempt - 1];
            if (retryDelayMs === undefined) break;

            logWeatherEvent("retry-scheduled", {
              attempt,
              nextAttempt: attempt + 1,
              delayMs: retryDelayMs,
              message: weatherErrorMessage(error),
            });
            if (!(await waitForRetry(retryDelayMs))) return;
          }
        }

        logWeatherEvent("retry-exhausted", {
          attempts: WEATHER_MAX_ATTEMPTS,
          message: weatherErrorMessage(lastError),
        });
        logWeatherEvent("load-failed", {
          attempts: WEATHER_MAX_ATTEMPTS,
          message: weatherErrorMessage(lastError),
        });
        if (!disposed) setWeather({ status: "unavailable", data: null });
      } finally {
        inFlight = false;
      }
    };

    void loadWeather();
    const weatherTimer = window.setInterval(
      () => void loadWeather(),
      WEATHER_REFRESH_MS,
    );

    return () => {
      disposed = true;
      window.clearInterval(weatherTimer);
      if (retryTimer !== null) window.clearTimeout(retryTimer);
      retryTimer = null;
      resolveRetry?.(false);
      resolveRetry = null;
    };
  }, []);

  useEffect(() => {
    let clockTimer: number | null = null;

    const scheduleClockUpdate = () => {
      const current = new Date();
      setNow(current);
      const elapsedInMinute =
        current.getSeconds() * 1000 + current.getMilliseconds();
      clockTimer = window.setTimeout(
        scheduleClockUpdate,
        60_000 - elapsedInMinute + 25,
      );
    };

    const handleVisibilityChange = () => {
      if (document.visibilityState !== "visible") return;
      if (clockTimer !== null) window.clearTimeout(clockTimer);
      scheduleClockUpdate();
    };

    scheduleClockUpdate();
    document.addEventListener("visibilitychange", handleVisibilityChange);

    return () => {
      if (clockTimer !== null) window.clearTimeout(clockTimer);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, []);

  const current = weather.data;
  const currentDescription = current
    ? describeWeatherCode(current.weatherCode)
    : "Weather unavailable";
  const forecastLabel = current
    ? current.forecast
        .map(
          (day) =>
            `${formatForecastDay(day.date)} ${Math.round(day.maxTemperature)}°/${Math.round(day.minTemperature)}°`,
        )
        .join(" · ")
    : "Weather forecast unavailable";

  return (
    <div
      className="header-weather"
      tabIndex={0}
      aria-label={`${currentDescription}. ${forecastLabel}`}
    >
      <div className="header-weather-current">
        <WeatherIcon code={current?.weatherCode ?? 0} />
        <strong className="header-weather-temperature">
          {current ? `${Math.round(current.temperature)}°` : "—°"}
        </strong>
        <span className="header-weather-divider" aria-hidden="true" />
        <time className="header-weather-time">
          {now.toLocaleTimeString(undefined, {
            hour: "numeric",
            minute: "2-digit",
          })}
        </time>
      </div>
      {current && (
        <div className="header-weather-forecast" aria-hidden="true">
          {current.forecast.map((day) => (
            <span key={day.date} className="header-weather-forecast-day">
              <span>{formatForecastDay(day.date)}</span>
              <WeatherIcon code={day.weatherCode} />
              <strong>
                {Math.round(day.maxTemperature)}° /{" "}
                {Math.round(day.minTemperature)}°
              </strong>
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

async function getCurrentPosition(): Promise<WeatherCoordinates> {
  if (navigator.geolocation) {
    try {
      const position = await getBrowserPosition();
      return {
        latitude: position.coords.latitude,
        longitude: position.coords.longitude,
        source: "browser",
      };
    } catch (error) {
      logWeatherEvent("geolocation-failed", {
        code: isRecord(error) ? error.code : undefined,
        message: weatherErrorMessage(error),
      });
    }
  } else {
    logWeatherEvent("geolocation-unavailable", {
      message: "Geolocation unavailable",
    });
  }

  logWeatherEvent("ip-geolocation-start");
  try {
    const response = await fetch("https://ipapi.co/json/", {
      cache: "no-store",
    });
    logWeatherEvent("ip-geolocation-response", {
      status: response.status,
      ok: response.ok,
    });
    if (!response.ok)
      throw new Error(`IP geolocation request failed (${response.status})`);
    const payload: unknown = await response.json();
    if (!isRecord(payload)) throw new Error("Invalid IP geolocation response");
    const latitude = readCoordinate(payload.latitude);
    const longitude = readCoordinate(payload.longitude);
    if (latitude === null || longitude === null) {
      throw new Error("Incomplete IP geolocation response");
    }
    logWeatherEvent("ip-geolocation-ready", {
      latitude: roundCoordinate(latitude),
      longitude: roundCoordinate(longitude),
    });
    return { latitude, longitude, source: "ip" };
  } catch (error) {
    logWeatherEvent("ip-geolocation-failed", {
      message: weatherErrorMessage(error),
    });
    throw new Error(`Location unavailable: ${weatherErrorMessage(error)}`);
  }
}

function getBrowserPosition(): Promise<GeolocationPosition> {
  return new Promise((resolve, reject) => {
    if (!navigator.geolocation) {
      reject(new Error("Geolocation unavailable"));
      return;
    }
    navigator.geolocation.getCurrentPosition(resolve, reject, {
      enableHighAccuracy: false,
      maximumAge: WEATHER_REFRESH_MS,
      timeout: 15_000,
    });
  });
}

function logWeatherEvent(
  event: string,
  details?: Record<string, unknown>,
): void {
  const logger =
    event.includes("failed") || event.includes("unavailable")
      ? console.warn
      : console.info;
  if (details) {
    logger(`[weather] ${event}`, details);
  } else {
    logger(`[weather] ${event}`);
  }
  if (!isTauriRuntime()) return;
  void invoke("record_weather_event", {
    event,
    details: details ? (JSON.stringify(details) ?? "") : "",
  }).catch((error: unknown) => {
    console.warn("[weather] persistent-log-failed", error);
  });
}

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function weatherErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (isRecord(error) && typeof error.message === "string") {
    return error.message;
  }
  return String(error);
}

function roundCoordinate(value: number): number {
  return Math.round(value * 1000) / 1000;
}

function parseWeatherResponse(payload: unknown): WeatherData {
  if (!isRecord(payload) || !isRecord(payload.current)) {
    throw new Error("Invalid current weather response");
  }
  const daily = isRecord(payload.daily) ? payload.daily : null;
  const temperature = readNumber(payload.current.temperature_2m);
  const weatherCode = readNumber(payload.current.weather_code);
  if (temperature === null || weatherCode === null || !daily) {
    throw new Error("Incomplete weather response");
  }

  const dates = readStringArray(daily.time);
  const codes = readNumberArray(daily.weather_code);
  const maxTemperatures = readNumberArray(daily.temperature_2m_max);
  const minTemperatures = readNumberArray(daily.temperature_2m_min);
  if (
    !dates ||
    !codes ||
    !maxTemperatures ||
    !minTemperatures ||
    dates.length !== codes.length ||
    dates.length !== maxTemperatures.length ||
    dates.length !== minTemperatures.length
  ) {
    throw new Error("Incomplete weather forecast response");
  }

  return {
    temperature,
    weatherCode,
    forecast: dates.map((date, index) => ({
      date,
      weatherCode: codes[index] ?? 0,
      maxTemperature: maxTemperatures[index] ?? 0,
      minTemperature: minTemperatures[index] ?? 0,
    })),
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object";
}

function readNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function readCoordinate(value: unknown): number | null {
  if (typeof value === "number") return Number.isFinite(value) ? value : null;
  if (typeof value !== "string" || value.trim() === "") return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function readNumberArray(value: unknown): number[] | null {
  if (!Array.isArray(value)) return null;
  const values = value.map(readNumber);
  return values.every((item): item is number => item !== null) ? values : null;
}

function readStringArray(value: unknown): string[] | null {
  if (!Array.isArray(value)) return null;
  return value.every((item): item is string => typeof item === "string")
    ? value
    : null;
}

function formatForecastDay(date: string): string {
  return new Intl.DateTimeFormat(undefined, { weekday: "short" }).format(
    new Date(`${date}T12:00:00`),
  );
}

function describeWeatherCode(code: number): string {
  if (code === 0) return "Clear sky";
  if (code <= 3) return "Partly cloudy";
  if (code <= 48) return "Foggy";
  if (code <= 67 || (code >= 80 && code <= 82)) return "Rain showers";
  if (code <= 77) return "Snow";
  if (code >= 95) return "Thunderstorm";
  return "Cloudy";
}

function WeatherIcon({ code }: { code: number }) {
  const isRain = code >= 51 && code <= 67;
  const isSnow = code >= 71 && code <= 77;
  const isStorm = code >= 95;
  const isCloudy = code >= 2 && code <= 48;

  return (
    <svg
      className="header-weather-icon"
      viewBox="0 0 32 32"
      fill="none"
      stroke="currentColor"
      strokeLinecap="round"
      strokeLinejoin="round"
      strokeWidth="2.5"
      aria-hidden="true"
    >
      {!isCloudy && !isRain && !isSnow && !isStorm && (
        <>
          <circle cx="16" cy="16" r="5" />
          <path d="M16 3v4M16 25v4M3 16h4M25 16h4M6.8 6.8l2.8 2.8M22.4 22.4l2.8 2.8M25.2 6.8l-2.8 2.8M9.6 22.4l-2.8 2.8" />
        </>
      )}
      {isCloudy && (
        <>
          <path d="M8 23h15a5 5 0 0 0 .5-10 7 7 0 0 0-13.4-1A5 5 0 0 0 8 23Z" />
          {code <= 3 && <path d="M8 8V4M5 6l3 2M11 6 8 8" />}
        </>
      )}
      {isRain && (
        <>
          <path d="M7 19h16a5 5 0 0 0 .5-10 7 7 0 0 0-13.4-1A5 5 0 0 0 7 19Z" />
          <path d="m10 23-1 3M16 23l-1 3M22 23l-1 3" />
        </>
      )}
      {isSnow && (
        <>
          <path d="M7 17h16a5 5 0 0 0 .5-10 7 7 0 0 0-13.4-1A5 5 0 0 0 7 17Z" />
          <path d="M11 23h.01M16 26h.01M21 23h.01" />
        </>
      )}
      {isStorm && (
        <>
          <path d="M7 17h16a5 5 0 0 0 .5-10 7 7 0 0 0-13.4-1A5 5 0 0 0 7 17Z" />
          <path d="m16 18-3 6h3l-1 5 5-8h-3l2-3" />
        </>
      )}
    </svg>
  );
}
