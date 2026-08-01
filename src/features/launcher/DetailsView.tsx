import { useEffect, useState } from "react";
import type { Game } from "../catalog/game-types";
import { useNavigation } from "../../ui/navigation/navigation-context";
import { useProductStore } from "../../stores/product-store";
import { Focusable } from "../../ui/navigation/focus/Focusable";

export function DetailsView({ game }: { game: Game | undefined }) {
  const { engine } = useNavigation();
  const closeDetails = useProductStore((state) => state.closeDetails);
  const activeView = useProductStore((state) => state.activeView);
  const [message, setMessage] = useState("Ready when you are.");

  useEffect(() => {
    if (activeView === "details" && game) engine.focus("details-play");
  }, [activeView, engine, game]);

  if (!game) return <p className="empty-state">Game not found.</p>;

  return (
    <section
      className="product-page details-view"
      aria-labelledby="details-heading"
    >
      <div className="details-hero">
        <img src={game.coverUrl} alt="" className="details-cover" />
        <div className="details-copy">
          <p className="eyebrow">
            {game.provider} · {game.platform}
          </p>
          <h1 id="details-heading">{game.title}</h1>
          <p>{game.description}</p>
          <div className="details-tags">
            {game.genres.map((genre) => (
              <span key={genre}>{genre}</span>
            ))}
            <span>{game.releaseYear}</span>
          </div>
          <div className="details-actions">
            <Focusable
              focusId="details-play"
              scopeId="product-shell"
              className="primary-button"
              onConfirm={() =>
                setMessage("Play simulated — no process was launched.")
              }
            >
              {game.installed ? "Play" : "Add to library"}
            </Focusable>
            <Focusable
              focusId="details-back"
              scopeId="product-shell"
              className="secondary-button"
              onConfirm={closeDetails}
            >
              Back
            </Focusable>
          </div>
          <p className="details-message" aria-live="polite">
            {message}
          </p>
        </div>
      </div>
      <div className="details-stats">
        <span>
          <strong>{game.progress}%</strong> progress
        </span>
        <span>
          <strong>{game.playtimeMinutes}m</strong> played
        </span>
        <span>
          <strong>{game.status}</strong> status
        </span>
      </div>
    </section>
  );
}
