import Hls from "hls.js";
import { useEffect, useRef, useState } from "react";

type SteamTrailerProps = {
  gameId: string;
  title: string;
  sourceUrls: string[];
  posterUrl: string;
};

export function SteamTrailer({
  gameId,
  title,
  sourceUrls,
  posterUrl,
}: SteamTrailerProps) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const [hasError, setHasError] = useState(false);

  useEffect(() => {
    setHasError(false);
  }, [gameId]);

  useEffect(() => {
    const video = videoRef.current;
    const sourceUrl = sourceUrls.find(isHlsSource) ?? sourceUrls[0];
    if (!video || !sourceUrl) return;

    let hls: Hls | undefined;
    const play = () => {
      void video.play().catch(() => {
        // Autoplay can be rejected by the webview; the muted video remains available.
      });
    };

    if (isHlsSource(sourceUrl)) {
      if (Hls.isSupported()) {
        hls = new Hls({
          enableWorker: true,
          lowLatencyMode: false,
          capLevelToPlayerSize: false,
        });
        hls.on(Hls.Events.ERROR, (_event, data) => {
          if (data.fatal) {
            setHasError(true);
          }
        });
        hls.on(Hls.Events.MANIFEST_PARSED, () => {
          if (hls && hls.levels.length > 0) {
            hls.currentLevel = hls.levels.length - 1;
          }
          play();
        });
        hls.loadSource(sourceUrl);
        hls.attachMedia(video);
      } else if (video.canPlayType("application/vnd.apple.mpegurl")) {
        video.src = sourceUrl;
        video.addEventListener("loadedmetadata", play, { once: true });
      } else {
        setHasError(true);
      }
    } else {
      video.src = sourceUrl;
      video.addEventListener("loadedmetadata", play, { once: true });
    }

    return () => {
      hls?.destroy();
      video.pause();
      video.removeAttribute("src");
      video.load();
    };
  }, [sourceUrls, gameId]);

  if (sourceUrls.length === 0 || hasError) return null;

  return (
    <div className="details-trailer" aria-hidden="true">
      <video
        key={gameId}
        ref={videoRef}
        className="details-trailer-video"
        autoPlay
        muted
        loop
        playsInline
        preload="auto"
        poster={posterUrl || undefined}
        onError={() => setHasError(true)}
        tabIndex={-1}
        disablePictureInPicture
      />
      <span className="visually-hidden">Trailer de {title}</span>
    </div>
  );
}

function isHlsSource(sourceUrl: string): boolean {
  return /\.m3u8(?:$|[?#])/i.test(sourceUrl);
}
