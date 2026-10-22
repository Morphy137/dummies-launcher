import {
  Image20Regular,
  MoreHorizontal20Regular,
  Play20Filled,
  Star20Filled,
  Star20Regular,
  Stop20Filled,
} from "@fluentui/react-icons";
import { type CSSProperties, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Game } from "../types";

interface PosterCardProps {
  game: Game;
  busy: boolean;
  onToggleRunning: (game: Game) => void;
}

type ArtStyle = CSSProperties & {
  "--art-a": string;
  "--art-b": string;
};

export default function PosterCard({ game, busy, onToggleRunning }: PosterCardProps) {
  const { t } = useTranslation();
  const [favorite, setFavorite] = useState(game.favorite);

  const artStyle: ArtStyle = {
    "--art-a": game.art[0],
    "--art-b": game.art[1],
  };

  return (
    <article className={`poster-card${game.running ? " is-running" : ""}`}>
      <div className="poster-card__art" style={artStyle}>
        <span className="poster-card__placeholder" aria-hidden="true">
          <Image20Regular />
          <span className="poster-card__initials">{game.initials}</span>
        </span>

        <button
          type="button"
          className="poster-card__open"
          aria-label={game.name}
          title={game.name}
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
          className={`favorite-button${favorite ? " is-active" : ""}`}
          onClick={() => setFavorite((current) => !current)}
          aria-label={favorite ? t("game.unfavorite") : t("game.favorite")}
          title={favorite ? t("game.unfavorite") : t("game.favorite")}
        >
          {favorite ? <Star20Filled /> : <Star20Regular />}
        </button>

        <div className="poster-card__overlay">
          <button
            type="button"
            className="play-button"
            onClick={() => onToggleRunning(game)}
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
