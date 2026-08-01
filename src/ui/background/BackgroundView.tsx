import { useEffect, useMemo, useSyncExternalStore } from "react";
import { BackgroundManager } from "./background-manager";

interface BackgroundViewProps {
  url: string | null;
  preloadUrls?: readonly (string | null)[];
}

export function BackgroundView({ url, preloadUrls = [] }: BackgroundViewProps) {
  const manager = useMemo(() => new BackgroundManager(), []);
  const snapshot = useSyncExternalStore(
    manager.subscribe,
    manager.getSnapshot,
    manager.getSnapshot,
  );

  useEffect(() => {
    manager.request(url);
    manager.preload(preloadUrls);
  }, [manager, preloadUrls, url]);

  useEffect(() => () => manager.dispose(), [manager]);

  return (
    <div className="background-view" aria-hidden="true">
      <div
        className="background-layer background-layer-current"
        style={
          snapshot.currentUrl
            ? { backgroundImage: `url("${snapshot.currentUrl}")` }
            : undefined
        }
      />
      <div
        className={`background-layer background-layer-incoming${snapshot.incomingVisible ? " is-visible" : ""}`}
        style={
          snapshot.incomingUrl
            ? { backgroundImage: `url("${snapshot.incomingUrl}")` }
            : undefined
        }
      />
      <div className="background-vignette" />
    </div>
  );
}
