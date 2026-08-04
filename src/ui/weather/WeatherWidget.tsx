import { useEffect, useState } from "react";

const WEATHER_REFRESH_MS = 15 * 60 * 1000;
const CLOCK_REFRESH_MS = 30 * 1000;

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

    const loadWeather = async () => {
      try {
        const position = await getCurrentPosition();
        const url = new URL("https://api.open-meteo.com/v1/forecast");
        url.searchParams.set("latitude", String(position.coords.latitude));
        url.searchParams.set("longitude", String(position.coords.longitude));
        url.searchParams.set("current", "temperature_2m,weather_code");
        url.searchParams.set(
          "daily",
          "weather_code,temperature_2m_max,temperature_2m_min",
        );
        url.searchParams.set("forecast_days", "4");
        url.searchParams.set("temperature_unit", "celsius");
        url.searchParams.set("timezone", "auto");

        const response = await fetch(url);
        if (!response.ok) throw new Error("Weather request failed");
        const payload: unknown = await response.json();
        const data = parseWeatherResponse(payload);
        if (!disposed) setWeather({ status: "ready", data });
      } catch {
        if (!disposed) setWeather({ status: "unavailable", data: null });
      }
    };

    void loadWeather();
    const weatherTimer = window.setInterval(
      () => void loadWeather(),
      WEATHER_REFRESH_MS,
    );
    const clockTimer = window.setInterval(
      () => setNow(new Date()),
      CLOCK_REFRESH_MS,
    );

    return () => {
      disposed = true;
      window.clearInterval(weatherTimer);
      window.clearInterval(clockTimer);
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

function getCurrentPosition(): Promise<GeolocationPosition> {
  return new Promise((resolve, reject) => {
    if (!navigator.geolocation) {
      reject(new Error("Geolocation unavailable"));
      return;
    }
    navigator.geolocation.getCurrentPosition(resolve, reject, {
      enableHighAccuracy: false,
      maximumAge: WEATHER_REFRESH_MS,
      timeout: 10_000,
    });
  });
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
