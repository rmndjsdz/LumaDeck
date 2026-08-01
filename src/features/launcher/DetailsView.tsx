import { useState } from "react";
import type { Game } from "../catalog/game-types";
import { useProductStore } from "../../stores/product-store";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { FocusScope } from "../../ui/navigation/focus/FocusScope";

export function DetailsView({ game }: { game: Game | undefined }) {
  const closeDetails = useProductStore((state) => state.closeDetails);
  const [message, setMessage] = useState("Ready when you are.");

  if (!game) return <p className="empty-state">Game not found.</p>;

  return (
    <FocusScope
      scopeId="details"
      parentScopeId="product-shell"
      initialFocusId="details-play"
      restoreFocus
      rememberScroll
      trapFocus
      modal
      activateOnMount
      onBack={() => {
        closeDetails();
        return true;
      }}
    >
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
                scopeId="details"
                className="primary-button"
                onConfirm={() =>
                  setMessage("Play simulated — no process was launched.")
                }
              >
                {game.installed ? "Play" : "Add to library"}
              </Focusable>
              <Focusable
                focusId="details-back"
                scopeId="details"
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
    </FocusScope>
  );
}
