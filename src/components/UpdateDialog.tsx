import {
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
} from "@fluentui/react-components";
import type { Update } from "@tauri-apps/plugin-updater";
import { useTranslation } from "react-i18next";

interface UpdateDialogProps {
  update: Update;
  installing: boolean;
  progress?: number;
  error: string | null;
  onInstall: () => void;
  onLater: () => void;
}

export default function UpdateDialog({
  update,
  installing,
  progress,
  error,
  onInstall,
  onLater,
}: UpdateDialogProps) {
  const { t } = useTranslation();

  return (
    <Dialog
      open
      onOpenChange={(_, data) => {
        if (!data.open && !installing) onLater();
      }}
    >
      <DialogSurface className="settings-dialog update-dialog">
        <DialogBody className="settings-body">
          <DialogTitle className="settings-title">{t("updates.title")}</DialogTitle>
          <DialogContent className="settings-content">
            <p className="update-copy">{t("updates.body", { version: update.version })}</p>

            {update.body ? <pre className="update-notes">{update.body}</pre> : null}

            {installing ? (
              <div className="update-progress" role="status" aria-live="polite">
                <div className="update-progress__track">
                  <div className="update-progress__bar" style={{ width: `${progress ?? 0}%` }} />
                </div>
                <span>
                  {progress === undefined
                    ? t("updates.downloadingUnknown")
                    : t("updates.downloading", { percent: progress })}
                </span>
              </div>
            ) : null}

            {error ? (
              <div className="catalog-error">
                {t("updates.failed")} {error}
              </div>
            ) : null}
          </DialogContent>

          <DialogActions className="settings-actions">
            <button
              type="button"
              className="secondary-button"
              onClick={onLater}
              disabled={installing}
            >
              {t("updates.later")}
            </button>
            <button
              type="button"
              className="primary-button"
              onClick={onInstall}
              disabled={installing}
            >
              {installing ? t("updates.installing") : t("updates.installNow")}
            </button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
