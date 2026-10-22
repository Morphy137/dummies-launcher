import {
  Dismiss16Regular,
  Square16Regular,
  SquareMultiple16Regular,
  Subtract16Regular,
} from "@fluentui/react-icons";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

export default function TitleBar() {
  const { t } = useTranslation();
  const [appWindow] = useState(() => (isTauri() ? getCurrentWindow() : null));
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!appWindow) return;

    let unlisten: (() => void) | undefined;
    const syncMaximized = () => {
      void appWindow
        .isMaximized()
        .then(setMaximized)
        .catch(() => {});
    };

    syncMaximized();
    void appWindow
      .onResized(syncMaximized)
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});

    return () => unlisten?.();
  }, [appWindow]);

  const toggleMaximize = () => {
    void appWindow?.toggleMaximize();
  };

  return (
    <header className="title-bar">
      {/* biome-ignore lint/a11y/noStaticElementInteractions: the drag region mirrors the native title bar double-click behavior. */}
      <div className="title-bar__drag" data-tauri-drag-region onDoubleClick={toggleMaximize}>
        <span className="app-mark" data-tauri-drag-region aria-hidden="true">
          <img src="/app-icon.svg" alt="" />
        </span>
        <span className="title-bar__title" data-tauri-drag-region>
          {t("app.title")}
        </span>
      </div>

      <div className="window-controls">
        <button
          type="button"
          className="window-control"
          onClick={() => void appWindow?.minimize()}
          aria-label={t("titlebar.minimize")}
          title={t("titlebar.minimize")}
        >
          <Subtract16Regular />
        </button>
        <button
          type="button"
          className="window-control"
          onClick={toggleMaximize}
          aria-label={maximized ? t("titlebar.restore") : t("titlebar.maximize")}
          title={maximized ? t("titlebar.restore") : t("titlebar.maximize")}
        >
          {maximized ? <SquareMultiple16Regular /> : <Square16Regular />}
        </button>
        <button
          type="button"
          className="window-control window-control--close"
          onClick={() => void appWindow?.close()}
          aria-label={t("titlebar.close")}
          title={t("titlebar.close")}
        >
          <Dismiss16Regular />
        </button>
      </div>
    </header>
  );
}
