import {
  ArrowLeft20Regular,
  Delete20Regular,
  Edit20Regular,
  Image20Regular,
  Open20Regular,
  Play20Filled,
  Settings20Regular,
  Star20Filled,
  Star20Regular,
  Stop20Filled,
} from "@fluentui/react-icons";
import { type CSSProperties, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { getFallbackArt, getGameCoverCandidates, getGameInitials } from "../lib/covers";
import type { Game } from "../types";

interface GameDetailProps {
  game: Game;
  busy: boolean;
  onBack: () => void;
  onConfigureSteam: (game: Game) => void;
  onCreateShortcut: (game: Game) => Promise<void>;
  onDelete: (game: Game) => Promise<void>;
  onToggleFavorite: (game: Game) => void;
  onToggleRunning: (game: Game) => void;
  onUpdateExe: (game: Game, exe: string) => Promise<void>;
}

export default function GameDetail({
  game,
  busy,
  onBack,
  onConfigureSteam,
  onCreateShortcut,
  onDelete,
  onToggleFavorite,
  onToggleRunning,
  onUpdateExe,
}: GameDetailProps) {
  const { t } = useTranslation();
  const [coverIndex, setCoverIndex] = useState(0);
  const [coverFailed, setCoverFailed] = useState(false);
  const [editingExe, setEditingExe] = useState(false);
  const [exeDraft, setExeDraft] = useState(game.exe);
  const exeInputRef = useRef<HTMLInputElement>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [deleting, setDeleting] = useState(false);

  const candidates = getGameCoverCandidates(game);
  const cover = candidates[coverIndex];
  const fallback = getFallbackArt(game.name);
  const initials = getGameInitials(game.name);
  const steamStatus = game.useSteamPath
    ? t("detail.steamConfigured")
    : game.requiresSteam
      ? t("detail.steamRequired")
      : game.steamAppId
        ? t("detail.steamOptional")
        : t("detail.steamUnavailable");

  const handleCoverError = () => {
    if (coverIndex + 1 < candidates.length) {
      setCoverIndex((current) => current + 1);
    } else {
      setCoverFailed(true);
    }
  };

  useEffect(() => {
    if (editingExe) {
      exeInputRef.current?.focus();
      exeInputRef.current?.select();
    }
  }, [editingExe]);

  const startEditingExe = () => {
    setExeDraft(game.exe);
    setEditingExe(true);
  };

  const saveExe = async () => {
    const next = exeDraft.trim();
    if (!next || next === game.exe) {
      setEditingExe(false);
      return;
    }
    try {
      await onUpdateExe(game, next);
      setEditingExe(false);
    } catch {
      // The parent reports the error through the toast.
    }
  };

  const deleteGame = async () => {
    setDeleting(true);
    try {
      await onDelete(game);
    } catch {
      // The parent reports the error through the toast.
    } finally {
      setDeleting(false);
    }
  };

  return (
    <div className="detail">
      <button type="button" className="detail__back" onClick={onBack}>
        <ArrowLeft20Regular />
        {t("detail.back")}
      </button>

      <section
        className="detail__hero"
        style={
          {
            "--art-a": fallback[0],
            "--art-b": fallback[1],
          } as CSSProperties
        }
      >
        <div
          className="detail__backdrop"
          style={{ backgroundImage: cover && !coverFailed ? `url(${cover})` : undefined }}
        />
        <div className="detail__scrim" />
        <div className="detail__hero-content">
          <div className="detail__poster">
            {cover && !coverFailed ? (
              <img src={cover} alt="" onError={handleCoverError} />
            ) : (
              <span className="poster-card__placeholder" aria-hidden="true">
                <Image20Regular />
                <span className="poster-card__initials">{initials}</span>
              </span>
            )}
          </div>

          <div className="detail__info">
            <div className="detail__eyebrow">
              <span className={`status-dot${game.running ? " is-active" : ""}`} />
              {game.running ? t("game.running") : t("detail.ready")}
            </div>
            <h1>{game.name}</h1>
            <div className="detail__meta">
              <span>{game.exe}</span>
              <span>{t("detail.appId", { id: game.appId })}</span>
            </div>
            <div className="detail__actions">
              <button
                type="button"
                className={`primary-button${game.running ? " is-danger" : ""}`}
                onClick={() => onToggleRunning(game)}
                disabled={busy}
              >
                {game.running ? <Stop20Filled /> : <Play20Filled />}
                {game.running ? t("game.stop") : t("game.play")}
              </button>
              <button
                type="button"
                className={`secondary-button detail__favorite${game.isFavorite ? " is-favorite" : ""}`}
                onClick={() => onToggleFavorite(game)}
                aria-label={game.isFavorite ? t("game.unfavorite") : t("game.favorite")}
                title={game.isFavorite ? t("game.unfavorite") : t("game.favorite")}
              >
                {game.isFavorite ? <Star20Filled /> : <Star20Regular />}
              </button>
            </div>
          </div>
        </div>
      </section>

      <section className="detail__panel">
        <div className="settings-section-title">{t("detail.details")}</div>

        <div className="detail__row">
          <span className="detail__label">{t("detail.appIdLabel")}</span>
          <span className="detail__value">{game.appId}</span>
        </div>

        <div className="detail__row">
          <span className="detail__label">{t("detail.executable")}</span>
          {editingExe ? (
            <div className="detail__exe-edit">
              <input
                ref={exeInputRef}
                className="detail__input"
                value={exeDraft}
                onChange={(event) => setExeDraft(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") void saveExe();
                  if (event.key === "Escape") setEditingExe(false);
                }}
                spellCheck={false}
              />
              <button type="button" className="secondary-button" onClick={() => void saveExe()}>
                {t("common.save")}
              </button>
              <button
                type="button"
                className="secondary-button"
                onClick={() => setEditingExe(false)}
              >
                {t("common.cancel")}
              </button>
            </div>
          ) : (
            <div className="detail__value-row">
              <span className="detail__value detail__mono">{game.exe}</span>
              <button
                type="button"
                className="icon-button icon-button--small"
                onClick={startEditingExe}
                disabled={game.running}
                aria-label={t("detail.editExe")}
                title={t("detail.editExe")}
              >
                <Edit20Regular />
              </button>
            </div>
          )}
        </div>

        <div className="detail__row">
          <span className="detail__label">{t("detail.running")}</span>
          <span className="detail__value">{game.running ? t("detail.yes") : t("detail.no")}</span>
        </div>

        <div className="detail__row">
          <span className="detail__label">{t("detail.steam")}</span>
          <div className="detail__value-row">
            <span
              className={`detail__value${game.requiresSteam && !game.useSteamPath ? " is-warning" : ""}`}
            >
              {steamStatus}
            </span>
            {game.steamAppId || game.requiresSteam || game.useSteamPath ? (
              <button
                type="button"
                className="icon-button icon-button--small"
                onClick={() => onConfigureSteam(game)}
                aria-label={t("steam.title")}
                title={t("steam.title")}
              >
                <Settings20Regular />
              </button>
            ) : null}
          </div>
        </div>

        <div className="detail__row">
          <span className="detail__label">{t("detail.shortcut")}</span>
          <button
            type="button"
            className="secondary-button"
            onClick={() => void onCreateShortcut(game)}
          >
            <Open20Regular />
            {t("detail.createShortcut")}
          </button>
        </div>

        <div className="detail__danger">
          {confirmDelete ? (
            <div className="detail__confirm">
              <span>{t("detail.confirmDelete", { name: game.name })}</span>
              <button
                type="button"
                className="secondary-button detail__danger-button"
                onClick={() => void deleteGame()}
                disabled={deleting}
              >
                {deleting ? t("detail.deleting") : t("detail.delete")}
              </button>
              <button
                type="button"
                className="secondary-button"
                onClick={() => setConfirmDelete(false)}
                disabled={deleting}
              >
                {t("common.cancel")}
              </button>
            </div>
          ) : (
            <button
              type="button"
              className="secondary-button detail__danger-button"
              onClick={() => setConfirmDelete(true)}
              disabled={game.running}
            >
              <Delete20Regular />
              {t("detail.delete")}
            </button>
          )}
        </div>
      </section>
    </div>
  );
}
