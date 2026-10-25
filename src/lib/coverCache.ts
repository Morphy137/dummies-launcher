const CACHE_NAME = "dummies-cover-cache-v1";
const objectUrls = new Map<string, string>();
const pendingRequests = new Map<string, Promise<string>>();
const references = new Map<string, number>();
const evictions = new Map<string, ReturnType<typeof setTimeout>>();
const MAX_COVERS = 128;
const MAX_AGE_MS = 7 * 24 * 60 * 60 * 1000;

export function retainCover(url: string): () => void {
  const timer = evictions.get(url);
  if (timer) clearTimeout(timer);
  evictions.delete(url);
  references.set(url, (references.get(url) ?? 0) + 1);
  return () => {
    const count = Math.max(0, (references.get(url) ?? 1) - 1);
    if (count) references.set(url, count);
    else {
      references.delete(url);
      scheduleEviction(url);
    }
  };
}

function scheduleEviction(url: string) {
  if (references.has(url) || evictions.has(url)) return;
  const timer = setTimeout(() => {
    const objectUrl = objectUrls.get(url);
    if (objectUrl) URL.revokeObjectURL(objectUrl);
    objectUrls.delete(url);
    evictions.delete(url);
  }, 30_000);
  evictions.set(url, timer);
}

function canUseCacheStorage(): boolean {
  return typeof caches !== "undefined" && typeof fetch === "function";
}

export function getCachedCoverUrl(url: string): string | undefined {
  return objectUrls.get(url);
}

async function loadCover(url: string): Promise<string> {
  if (!canUseCacheStorage() || typeof URL.createObjectURL !== "function") {
    return url;
  }

  try {
    const cache = await caches.open(CACHE_NAME);
    let response = await cache.match(url);
    if (
      response &&
      Date.now() - Number(response.headers.get("x-dummies-cached-at") ?? 0) > MAX_AGE_MS
    ) {
      await cache.delete(url);
      response = undefined;
    }

    if (!response) {
      response = await fetch(url, {
        mode: "cors",
        credentials: "omit",
        signal: AbortSignal.timeout(10_000),
      });
      if (!response.ok) {
        throw new Error(`Cover request failed with ${response.status}`);
      }
      try {
        await cache.put(
          url,
          new Response(await response.clone().blob(), {
            headers: {
              "content-type": response.headers.get("content-type") ?? "image/jpeg",
              "x-dummies-cached-at": String(Date.now()),
            },
          }),
        );
        const keys = await cache.keys();
        await Promise.all(
          keys.slice(0, Math.max(0, keys.length - MAX_COVERS)).map((key) => cache.delete(key)),
        );
      } catch {
        // The in-memory object URL still works if persistent storage is unavailable.
      }
    }

    const objectUrl = URL.createObjectURL(await response.blob());
    objectUrls.set(url, objectUrl);
    scheduleEviction(url);
    return objectUrl;
  } catch {
    // Let the <img> element try the original URL. This preserves the old behavior
    // for CDNs that do not allow fetch/CORS or when storage is unavailable.
    return url;
  }
}

export function getCoverUrl(url: string): Promise<string> {
  const cached = objectUrls.get(url);
  if (cached) return Promise.resolve(cached);

  const pending = pendingRequests.get(url);
  if (pending) return pending;

  const request = loadCover(url).finally(() => {
    pendingRequests.delete(url);
  });
  pendingRequests.set(url, request);
  return request;
}
