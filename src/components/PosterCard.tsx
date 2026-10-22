import {
  Image20Regular,
  MoreHorizontal20Regular,
  Play20Filled,
  Star20Filled,
  Star20Regular,
  Stop20Filled,
} from "@fluentui/react-icons";
import { type CSSProperties, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getFallbackArt, getGameCoverCandidates, getGameInitials } from "../lib/covers";
import type { Game } from "../types";

interface PosterCardProps {
  game: Game;
  busy: boolean;
  onOpen: (game: Game) => void;
  onToggleFavorite: (game: Game) => void;
  onToggleRunning: (game: Game) => void;
}

type ArtStyle = CSSProperties & {
  "--art-a": string;
  "--art-b": string;
};

export default function PosterCard({
  game,
  busy,
  onOpen,
  onToggleFavorite,
  onToggleRunning,
}: PosterCardProps) {
  const { t } = useTranslation();
  const [candidateIndex, setCandidateIndex] = useState(0);
  const [imageFailed, setImageFailed] = useState(false);

  const candidates = getGameCoverCandidates(game);
  const cover = candidates[candidateIndex];
  const fallback = getFallbackArt(game.name);
  const initials = getGameInitials(game.name);
  const artStyle: ArtStyle = {
    "--art-a": fallback[0],
    "--art-b": fallback[1],
  };

  // biome-ignore lint/correctness/useExhaustiveDependencies: reset the image fallback when cover metadata changes.
  useEffect(() => {
    setCandidateIndex(0);
    setImageFailed(false);
  }, [game.appId, game.coverImageHash, game.iconHash, game.steamAppId]);

  const handleImageError = () => {
    if (candidateIndex + 1 < candidates.length) {
      setCandidateIndex((current) => current + 1);
    } else {
      setImageFailed(true);
    }
  };

  return (
    <article className={`poster-card${game.running ? " is-running" : ""}`}>
      <div className="poster-card__art" style={artStyle}>
        {cover && !imageFailed ? (
          <img
            className="poster-card__poster"
            src={cover}
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

        {game.running ? (
          <span className="running-badge">
            <span className="status-dot" />
            <span>{t("game.running")}</span>
            {game.remaining ? <span className="running-badge__timer">{game.remaining}</span> : null}
          </span>
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
          {game.isFavorite ? <Star20Filled /> : <Star20Regular />}
        </button>

        <div className="poster-card__overlay">
          <button
            type="button"
            className="play-button"
            onClick={(event) => {
              event.stopPropagation();
              onToggleRunning(game);
            }}
            disabled={busy}
            aria-busy={busy}
            aria-label={game.running ? t("game.stop") : t("game.play")}
            title={game.running ? t("game.stop") : t("game.play")}
          >
            {game.running ? <Stop20Filled /> : <Play20Filled />}
          </button>
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
