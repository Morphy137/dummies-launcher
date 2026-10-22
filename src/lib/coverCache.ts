const CACHE_NAME = "dummies-cover-cache-v1";
const objectUrls = new Map<string, string>();
const pendingRequests = new Map<string, Promise<string>>();

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

    if (!response) {
      response = await fetch(url, { mode: "cors", credentials: "omit" });
      if (!response.ok) {
        throw new Error(`Cover request failed with ${response.status}`);
      }
      try {
        await cache.put(url, response.clone());
      } catch {
        // The in-memory object URL still works if persistent storage is unavailable.
      }
    }

    const objectUrl = URL.createObjectURL(await response.blob());
    objectUrls.set(url, objectUrl);
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
