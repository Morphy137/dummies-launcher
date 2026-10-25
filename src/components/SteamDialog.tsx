import {
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Spinner,
} from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getSteamLibraries } from "../lib/backend";
import { formatError } from "../lib/errors";
import type { Game, SteamSetupRequest } from "../types";

interface SteamDialogProps {
  open: boolean;
  game: Game | null;
  onOpenChange: (open: boolean) => void;
  onRemove: () => Promise<void>;
  onSave: (request: SteamSetupRequest) => Promise<void>;
}

function defaultInstallDir(game: Game): string {
  return (
    game.installDir ||
    game.name
      .replace(/[<>:"/\\|?*]/g, "_")
      .trim()
      .slice(0, 120)
  );
}

export default function SteamDialog({
  open,
  game,
  onOpenChange,
  onRemove,
  onSave,
}: SteamDialogProps) {
  const { t } = useTranslation();
  const [steamPath, setSteamPath] = useState<string | null>(null);
  const [libraries, setLibraries] = useState<string[]>([]);
  const [steamAppId, setSteamAppId] = useState("");
  const [installDir, setInstallDir] = useState("");
  const [exe, setExe] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmRemove, setConfirmRemove] = useState(false);

  useEffect(() => {
    if (!open || !game) return;

    let cancelled = false;
    setError(null);
    setConfirmRemove(false);
    setSteamPath(null);
    setSteamAppId(game.steamAppId || "");
    setInstallDir(defaultInstallDir(game));
    setExe(game.exe);

    void getSteamLibraries()
      .then((paths) => {
        if (!cancelled) {
          setLibraries(paths);
          setSteamPath(game.steamLibraryPath ?? paths[0] ?? null);
        }
      })
      .catch((reason: unknown) => {
        if (!cancelled) setError(formatError(reason));
      });

    return () => {
      cancelled = true;
    };
  }, [game, open]);

  const handleSave = async () => {
    if (!game || game.running) return;
    setBusy(true);
    setError(null);
    try {
      await onSave({
        appId: game.appId,
        currentExe: game.exe,
        libraryPath: steamPath,
        steamAppId: steamAppId.trim(),
        installDir: installDir.trim(),
        exe: exe.trim(),
      });
    } catch (saveError: unknown) {
      setError(formatError(saveError));
    } finally {
      setBusy(false);
    }
  };

  const handleRemove = async () => {
    if (game?.running) return;
    setBusy(true);
    setError(null);
    try {
      await onRemove();
    } catch (removeError: unknown) {
      setError(formatError(removeError));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(_, data) => {
        if (!busy) onOpenChange(data.open);
      }}
    >
      <DialogSurface className="steam-dialog">
        <DialogBody className="steam-body">
          <DialogTitle className="steam-title">{t("steam.title")}</DialogTitle>
          <DialogContent className="steam-content">
            <p className="steam-note">{t("steam.note")}</p>
            <p className="steam-note">{t("steam.safeChanges")}</p>
            {game?.running ? <p role="status">{t("steam.stopFirst")}</p> : null}
            <div className="steam-path">
              <span>{t("steam.path")}</span>
              {libraries.length > 0 ? (
                <select
                  value={steamPath ?? ""}
                  aria-label={t("steam.path")}
                  disabled={busy || game?.running || game?.useSteamPath}
                  onChange={(event) => setSteamPath(event.target.value)}
                >
                  {libraries.map((path) => (
                    <option key={path} value={path}>
                      {path}
                    </option>
                  ))}
                </select>
              ) : (
                <span>{t("steam.notFound")}</span>
              )}
            </div>

            <label className="steam-field">
              <span>{t("steam.appId")}</span>
              <input
                className="steam-input"
                disabled={busy || game?.running || game?.useSteamPath}
                value={steamAppId}
                onChange={(event) => setSteamAppId(event.target.value)}
                placeholder="3787240"
                spellCheck={false}
              />
            </label>

            <label className="steam-field">
              <span>{t("steam.installDir")}</span>
              <input
                className="steam-input"
                disabled={busy || game?.running || game?.useSteamPath}
                value={installDir}
                onChange={(event) => setInstallDir(event.target.value)}
                placeholder={t("steam.folderPlaceholder")}
                spellCheck={false}
              />
            </label>

            <label className="steam-field">
              <span>{t("steam.exe")}</span>
              <input
                className="steam-input"
                disabled={busy || game?.running || game?.useSteamPath}
                value={exe}
                onChange={(event) => setExe(event.target.value)}
                placeholder="Binaries/Win64/Game.exe"
                spellCheck={false}
              />
            </label>

            {error ? <div className="catalog-error">{error}</div> : null}
            {confirmRemove ? (
              <div role="alert">
                <p>{t("steam.confirmRemove")}</p>
                <button
                  type="button"
                  className="secondary-button"
                  disabled={busy || game?.running}
                  onClick={() => void handleRemove()}
                >
                  {t("steam.remove")}
                </button>
                <button
                  type="button"
                  className="secondary-button"
                  disabled={busy}
                  onClick={() => setConfirmRemove(false)}
                >
                  {t("common.cancel")}
                </button>
              </div>
            ) : null}
          </DialogContent>

          <DialogActions className="steam-actions">
            {game?.useSteamPath ? (
              <button
                type="button"
                className="secondary-button detail__danger-button"
                onClick={() => setConfirmRemove(true)}
                disabled={busy || game.running}
              >
                {t("steam.remove")}
              </button>
            ) : null}
            <span className="steam-actions__spacer" />
            <button
              type="button"
              className="secondary-button"
              onClick={() => onOpenChange(false)}
              disabled={busy}
            >
              {t("common.cancel")}
            </button>
            <button
              type="button"
              className="primary-button"
              onClick={() => void handleSave()}
              disabled={busy || !steamPath || game?.running || confirmRemove}
            >
              {busy ? <Spinner size="tiny" /> : null}
              {game?.useSteamPath ? t("steam.update") : t("steam.install")}
            </button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
