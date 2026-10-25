import {
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Spinner,
} from "@fluentui/react-components";
import { Add16Regular, Search20Regular } from "@fluentui/react-icons";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { getDatabasePage, syncGamelist } from "../lib/backend";
import { formatError } from "../lib/errors";
import type { DatabaseGame } from "../types";

interface AddGameDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAdd: (game: DatabaseGame) => Promise<void>;
}

const PAGE_SIZE = 60;
export default function AddGameDialog({ open, onOpenChange, onAdd }: AddGameDialogProps) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<DatabaseGame[]>([]);
  const [offset, setOffset] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const [addingKey, setAddingKey] = useState<string | null>(null);
  const attemptedSync = useRef(false);
  const syncBusy = useRef(false);
  const listRef = useRef<HTMLDivElement>(null);

  const refresh = useCallback(async () => {
    if (syncBusy.current) return;
    syncBusy.current = true;
    attemptedSync.current = true;
    setSyncing(true);
    setError(null);
    try {
      await syncGamelist();
      setRevision((value) => value + 1);
    } catch (reason: unknown) {
      setError(`${t("catalog.offline")} ${formatError(reason)}`);
    } finally {
      syncBusy.current = false;
      setSyncing(false);
    }
  }, [t]);

  useEffect(() => {
    if (open && !attemptedSync.current) void refresh();
  }, [open, refresh]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: a successful catalog sync invalidates the cached page.
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setLoading(true);
    const timer = window.setTimeout(
      () => {
        void getDatabasePage(query, offset, PAGE_SIZE)
          .then((page) => {
            if (cancelled) return;
            setItems(page.items);
            setHasMore(page.hasMore);
            if (listRef.current) listRef.current.scrollTop = 0;
          })
          .catch((reason: unknown) => {
            if (!cancelled) setError(formatError(reason));
          })
          .finally(() => {
            if (!cancelled) setLoading(false);
          });
      },
      query ? 250 : 0,
    );
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [open, query, offset, revision]);

  const handleAdd = async (game: DatabaseGame) => {
    if (addingKey) return;
    setAddingKey(`${game.id}::${game.exe}`);
    setError(null);
    try {
      await onAdd(game);
      onOpenChange(false);
    } catch (reason: unknown) {
      setError(formatError(reason));
    } finally {
      setAddingKey(null);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(_, data) => {
        if (!addingKey) onOpenChange(data.open);
      }}
    >
      <DialogSurface className="catalog-dialog">
        <DialogBody className="catalog-body">
          <DialogTitle className="catalog-title">{t("catalog.title")}</DialogTitle>
          <DialogContent className="catalog-content">
            <label className="catalog-search">
              <Search20Regular aria-hidden="true" />
              <input
                value={query}
                onChange={(event) => {
                  setQuery(event.target.value);
                  setOffset(0);
                  setItems([]);
                }}
                placeholder={t("catalog.searchPlaceholder")}
                aria-label={t("catalog.searchPlaceholder")}
                spellCheck={false}
                autoFocus
              />
            </label>
            {error ? (
              <div className="catalog-error" role="alert">
                {error}
              </div>
            ) : null}
            {syncing ? (
              <div role="status">
                <Spinner size="tiny" />
                {t("catalog.syncing")}
              </div>
            ) : null}
            <div className="catalog-list" ref={listRef} aria-busy={loading}>
              {items.map((game) => (
                <button
                  key={`${game.id}::${game.exe}`}
                  type="button"
                  className="catalog-row"
                  onClick={() => void handleAdd(game)}
                  disabled={addingKey !== null || loading}
                >
                  <span className="catalog-row__text">
                    <span className="catalog-row__name">{game.name}</span>
                    <span className="catalog-row__exe">{game.exe}</span>
                  </span>
                  {game.usesNewDetection ? (
                    <span className="catalog-badge">{t("catalog.fallbackExe")}</span>
                  ) : null}
                  <span className="catalog-row__add">
                    <Add16Regular />
                  </span>
                </button>
              ))}
              {loading ? (
                <div className="catalog-status" role="status">
                  {t("catalog.loading")}
                </div>
              ) : items.length === 0 ? (
                <div className="catalog-status">
                  {t(query ? "catalog.empty" : "catalog.noCache")}
                </div>
              ) : null}
            </div>
          </DialogContent>
          <DialogActions className="catalog-actions">
            <button
              type="button"
              className="secondary-button"
              onClick={() => void refresh()}
              disabled={syncing || !!addingKey}
            >
              {t("catalog.refresh")}
            </button>
            <button
              type="button"
              className="secondary-button"
              disabled={loading || offset === 0 || !!addingKey}
              onClick={() => setOffset((value) => Math.max(0, value - PAGE_SIZE))}
            >
              {t("catalog.previous")}
            </button>
            <button
              type="button"
              className="secondary-button"
              disabled={loading || !hasMore || !!addingKey}
              onClick={() => setOffset((value) => value + PAGE_SIZE)}
            >
              {t("catalog.next")}
            </button>
            <button
              type="button"
              className="secondary-button"
              disabled={!!addingKey}
              onClick={() => onOpenChange(false)}
            >
              {t("common.close")}
            </button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
