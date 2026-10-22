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
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { getDatabasePage, syncGamelist } from "../lib/backend";
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
  const [addingKey, setAddingKey] = useState<string | null>(null);
  const hasSynced = useRef(false);
  const requestId = useRef(0);

  useEffect(() => {
    if (!open) return;

    let cancelled = false;
    const currentRequest = ++requestId.current;
    const timer = window.setTimeout(
      async () => {
        setLoading(true);
        setError(null);

        try {
          if (!hasSynced.current) {
            setSyncing(true);
            try {
              await syncGamelist();
              hasSynced.current = true;
            } catch (syncError: unknown) {
              if (!cancelled) {
                setError(syncError instanceof Error ? syncError.message : String(syncError));
              }
            } finally {
              if (!cancelled) setSyncing(false);
            }
          }

          const page = await getDatabasePage(query, 0, PAGE_SIZE);
          if (cancelled || currentRequest !== requestId.current) return;
          setItems(page.items);
          setOffset(page.items.length);
          setHasMore(page.hasMore);
        } catch (loadError: unknown) {
          if (!cancelled) {
            setError(loadError instanceof Error ? loadError.message : String(loadError));
          }
        } finally {
          if (!cancelled) setLoading(false);
        }
      },
      query ? 250 : 0,
    );

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [open, query]);

  const loadMore = async () => {
    if (loading || !hasMore) return;
    setLoading(true);
    try {
      const page = await getDatabasePage(query, offset, PAGE_SIZE);
      setItems((current) => [...current, ...page.items]);
      setOffset((current) => current + page.items.length);
      setHasMore(page.hasMore);
    } catch (loadError: unknown) {
      setError(loadError instanceof Error ? loadError.message : String(loadError));
    } finally {
      setLoading(false);
    }
  };

  const handleAdd = async (game: DatabaseGame) => {
    const key = `${game.id}::${game.exe}`;
    setAddingKey(key);
    setError(null);
    try {
      await onAdd(game);
      onOpenChange(false);
    } catch (addError: unknown) {
      setError(addError instanceof Error ? addError.message : String(addError));
    } finally {
      setAddingKey(null);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(_, data) => onOpenChange(data.open)}>
      <DialogSurface className="catalog-dialog">
        <DialogBody className="catalog-body">
          <DialogTitle className="catalog-title">{t("catalog.title")}</DialogTitle>
          <DialogContent className="catalog-content">
            <label className="catalog-search">
              <Search20Regular aria-hidden="true" />
              <input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder={t("catalog.searchPlaceholder")}
                aria-label={t("catalog.searchPlaceholder")}
                spellCheck={false}
                autoFocus
              />
            </label>

            {error ? <div className="catalog-error">{error}</div> : null}

            <div className="catalog-list">
              {syncing ? (
                <div className="catalog-status">
                  <Spinner size="tiny" />
                  {t("catalog.syncing")}
                </div>
              ) : null}

              {items.map((game) => {
                const key = `${game.id}::${game.exe}`;
                return (
                  <button
                    key={key}
                    type="button"
                    className="catalog-row"
                    onClick={() => void handleAdd(game)}
                    disabled={addingKey !== null}
                  >
                    <span className="catalog-row__text">
                      <span className="catalog-row__name">{game.name}</span>
                      <span className="catalog-row__exe">{game.exe}</span>
                    </span>
                    {game.usesNewDetection ? (
                      <span className="catalog-badge">{t("catalog.fallbackExe")}</span>
                    ) : null}
                    <span className="catalog-row__add">
                      {addingKey === key ? <Spinner size="tiny" /> : <Add16Regular />}
                    </span>
                  </button>
                );
              })}

              {!syncing && !loading && items.length === 0 ? (
                <div className="catalog-status">{t("catalog.empty")}</div>
              ) : null}

              {hasMore ? (
                <button
                  type="button"
                  className="secondary-button catalog-more"
                  onClick={() => void loadMore()}
                  disabled={loading}
                >
                  {loading ? t("catalog.loading") : t("catalog.loadMore")}
                </button>
              ) : null}
            </div>
          </DialogContent>

          <DialogActions className="catalog-actions">
            <button type="button" className="secondary-button" onClick={() => onOpenChange(false)}>
              {t("common.close")}
            </button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
