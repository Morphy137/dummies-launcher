type Kind = "cover" | "banner";
type Entry = { urls: string[]; expires: number };

const STORAGE_KEY = "dummies-artwork-lookups-v1";
const MAX_ENTRIES = 512;
const MAX_AGE = 7 * 24 * 60 * 60_000;
const RETRY_DELAY = 5 * 60_000;
const entries = new Map<string, Entry>();
const pending = new Map<string, Promise<string[]>>();
let loaded = false;

function keyFor(kind: Kind, name: string, steamAppId?: string | null): string {
  return JSON.stringify([kind, name, steamAppId ?? null]);
}

function load() {
  if (loaded) return;
  loaded = true;
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]");
    if (!Array.isArray(saved)) return;
    for (const item of saved.slice(-MAX_ENTRIES)) {
      if (
        Array.isArray(item) &&
        typeof item[0] === "string" &&
        Array.isArray(item[1]?.urls) &&
        item[1].urls.every((url: unknown) => typeof url === "string") &&
        typeof item[1].expires === "number"
      ) {
        entries.set(item[0], item[1]);
      }
    }
  } catch {
    // Storage can be unavailable or contain data from an interrupted write.
  }
}

function save() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify([...entries]));
  } catch {
    // Keep lookups in memory if persistent storage is unavailable.
  }
}

export function cachedArtwork(kind: Kind, name: string, steamAppId?: string | null): string[] {
  load();
  return entries.get(keyFor(kind, name, steamAppId))?.urls ?? [];
}

export function lookupArtwork(
  kind: Kind,
  name: string,
  steamAppId: string | null | undefined,
  resolve: () => Promise<string[]>,
): Promise<string[]> {
  load();
  const key = keyFor(kind, name, steamAppId);
  const previous = entries.get(key);
  if (previous && previous.expires > Date.now()) return Promise.resolve(previous.urls);
  const existing = pending.get(key);
  if (existing) return existing;
  const request = Promise.resolve()
    .then(resolve)
    .catch(() => [])
    .then((urls) => {
      if (urls.length) {
        entries.delete(key);
        entries.set(key, { urls, expires: Date.now() + MAX_AGE });
        while (entries.size > MAX_ENTRIES) entries.delete(entries.keys().next().value as string);
        save();
        return urls;
      }
      if (previous) {
        // Retry later, while retaining the last known images for offline use.
        entries.set(key, { ...previous, expires: Date.now() + RETRY_DELAY });
        return previous.urls;
      }
      entries.set(key, { urls: [], expires: Date.now() + RETRY_DELAY });
      while (entries.size > MAX_ENTRIES) entries.delete(entries.keys().next().value as string);
      return [];
    })
    .finally(() => pending.delete(key));
  pending.set(key, request);
  return request;
}
