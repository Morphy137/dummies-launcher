import {
  Heart20Filled,
  Heart20Regular,
  Image20Regular,
  MoreHorizontal20Regular,
  Pause20Filled,
  Play20Filled,
  Stop20Filled,
} from "@fluentui/react-icons";
import type { CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import { getFallbackArt, getGameCoverCandidates, getGameInitials } from "../lib/covers";
import { useCoverCandidates } from "../lib/useCoverCandidates";
import type { Game } from "../types";
import RetroClock from "./RetroClock";

interface PosterCardProps {
  game: Game;
  busy: boolean;
  canPause: boolean;
  onOpen: (game: Game) => void;
  onToggleFavorite: (game: Game) => void;
  onToggleRunning: (game: Game) => void;
  onTogglePause: (game: Game) => void;
}

type ArtStyle = CSSProperties & {
  "--art-a": string;
  "--art-b": string;
};

export default function PosterCard({
  game,
  busy,
  canPause,
  onOpen,
  onToggleFavorite,
  onToggleRunning,
  onTogglePause,
}: PosterCardProps) {
  const { t } = useTranslation();
  const candidates = getGameCoverCandidates(game);
  const {
    src: cachedCover,
    failed: imageFailed,
    handleError: handleImageError,
  } = useCoverCandidates(candidates);
  const fallback = getFallbackArt(game.name);
  const initials = getGameInitials(game.name);
  const artStyle: ArtStyle = {
    "--art-a": fallback[0],
    "--art-b": fallback[1],
  };

  return (
    <article
      className={`poster-card${game.running ? " is-running" : ""}${game.paused ? " is-paused" : ""}`}
    >
      <div className="poster-card__art" style={artStyle}>
        {cachedCover && !imageFailed ? (
          <img
            className="poster-card__poster"
            src={cachedCover}
            alt=""
            loading="lazy"
            decoding="async"
            onError={handleImageError}
          />
        ) : (
          <span className="poster-card__placeholder" aria-hidden="true">
            <Image20Regular />
            <span className="poster-card__initials">{initials}</span>
          </span>
        )}

        <button
          type="button"
          className="poster-card__open"
          aria-label={game.name}
          title={game.name}
          onClick={() => onOpen(game)}
        />

        {game.running && game.remaining ? (
          <RetroClock time={game.remaining} paused={game.paused} />
        ) : null}

        <button
          type="button"
          className={`favorite-button${game.isFavorite ? " is-active" : ""}`}
          onClick={(event) => {
            event.stopPropagation();
            onToggleFavorite(game);
          }}
          aria-label={game.isFavorite ? t("game.unfavorite") : t("game.favorite")}
          title={game.isFavorite ? t("game.unfavorite") : t("game.favorite")}
        >
          {game.isFavorite ? <Heart20Filled /> : <Heart20Regular />}
        </button>

        <div className="poster-card__overlay">
          {game.running ? (
            <div className="poster-card__controls">
              {canPause ? (
                <button
                  type="button"
                  className="play-button"
                  onClick={(event) => {
                    event.stopPropagation();
                    onTogglePause(game);
                  }}
                  disabled={busy}
                  aria-label={game.paused ? t("game.resume") : t("game.pause")}
                  title={game.paused ? t("game.resume") : t("game.pause")}
                >
                  {game.paused ? <Play20Filled /> : <Pause20Filled />}
                </button>
              ) : null}
              <button
                type="button"
                className="play-button play-button--danger"
                onClick={(event) => {
                  event.stopPropagation();
                  onToggleRunning(game);
                }}
                disabled={busy}
                aria-busy={busy}
                aria-label={t("game.stop")}
                title={t("game.stop")}
              >
                <Stop20Filled />
              </button>
            </div>
          ) : (
            <button
              type="button"
              className="play-button"
              onClick={(event) => {
                event.stopPropagation();
                onToggleRunning(game);
              }}
              disabled={busy}
              aria-busy={busy}
              aria-label={t("game.play")}
              title={t("game.play")}
            >
              <Play20Filled />
            </button>
          )}
        </div>
      </div>

      <div className="poster-card__meta">
        <span className="poster-card__title" title={game.name}>
          {game.name}
        </span>
        <button
          type="button"
          className="more-button"
          aria-label={t("game.more")}
          title={t("game.more")}
        >
          <MoreHorizontal20Regular />
        </button>
      </div>
    </article>
  );
}
