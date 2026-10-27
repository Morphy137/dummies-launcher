const CACHE_NAME = "dummies-cover-cache-v1";
const objectUrls = new Map<string, { src: string; bytes: number }>();
const pendingRequests = new Map<string, Promise<string>>();
const references = new Map<string, number>();
const failures = new Map<string, number>();
const MAX_COVERS = 128;
const MAX_MEMORY_BYTES = 32 * 1024 * 1024;
const MAX_AGE_MS = 7 * 24 * 60 * 60 * 1000;

export function retainCover(url: string): () => void {
  references.set(url, (references.get(url) ?? 0) + 1);
  return () => {
    const count = Math.max(0, (references.get(url) ?? 1) - 1);
    if (count) references.set(url, count);
    else {
      references.delete(url);
      trimMemoryCache();
    }
  };
}

function trimMemoryCache(protectedUrl?: string) {
  let bytes = [...objectUrls.values()].reduce((total, entry) => total + entry.bytes, 0);
  for (const [url, entry] of objectUrls) {
    if (objectUrls.size <= MAX_COVERS && bytes <= MAX_MEMORY_BYTES) break;
    // Never revoke an image still mounted, or the image being delivered to a caller.
    if (references.has(url) || url === protectedUrl) continue;
    URL.revokeObjectURL(entry.src);
    objectUrls.delete(url);
    bytes -= entry.bytes;
  }
}

export function getCachedCoverUrl(url: string): string | undefined {
  const entry = objectUrls.get(url);
  if (entry) {
    objectUrls.delete(url);
    objectUrls.set(url, entry);
  }
  return entry?.src;
}

class UnavailableCover extends Error {}

async function loadCover(url: string): Promise<string> {
  if (typeof fetch !== "function" || typeof URL.createObjectURL !== "function") {
    return url;
  }

  let cache: Cache | undefined;
  let response: Response | undefined;
  let staleResponse: Response | undefined;
  try {
    if (typeof caches !== "undefined") cache = await caches.open(CACHE_NAME);
    response = await cache?.match(url);
    if (
      response &&
      Date.now() - Number(response.headers.get("x-dummies-cached-at") ?? 0) > MAX_AGE_MS
    ) {
      staleResponse = response;
      response = undefined;
    }
  } catch {
    // Memory caching must still work when disk storage is unavailable or denied.
    cache = undefined;
  }

  try {
    if (!response) {
      response = await fetch(url, {
        mode: "cors",
        credentials: "omit",
        signal: AbortSignal.timeout(10_000),
      });
      if (!response.ok) {
        failures.set(url, Date.now() + ([404, 410].includes(response.status) ? 600_000 : 60_000));
        if (failures.size > 256) {
          const oldest = failures.keys().next().value;
          if (oldest) failures.delete(oldest);
        }
        throw new UnavailableCover(`Cover request failed with ${response.status}`);
      }
      try {
        await cache?.put(
          url,
          new Response(await response.clone().blob(), {
            headers: {
              "content-type": response.headers.get("content-type") ?? "image/jpeg",
              "x-dummies-cached-at": String(Date.now()),
            },
          }),
        );
        const keys = (await cache?.keys()) ?? [];
        await Promise.all(
          keys.slice(0, Math.max(0, keys.length - MAX_COVERS)).map((key) => cache?.delete(key)),
        );
      } catch {
        // The in-memory object URL still works if persistent storage is unavailable.
      }
    }

    const blob = await response.blob();
    const objectUrl = URL.createObjectURL(blob);
    objectUrls.set(url, { src: objectUrl, bytes: blob.size });
    trimMemoryCache(url);
    return objectUrl;
  } catch (error: unknown) {
    if (staleResponse) {
      const blob = await staleResponse.blob();
      const objectUrl = URL.createObjectURL(blob);
      objectUrls.set(url, { src: objectUrl, bytes: blob.size });
      trimMemoryCache(url);
      return objectUrl;
    }
    if (error instanceof UnavailableCover) throw error;
    // Let the <img> element try the original URL. This preserves the old behavior
    // for CDNs that do not allow fetch/CORS or when storage is unavailable.
    return url;
  }
}

export function getCoverUrl(url: string): Promise<string> {
  const cached = getCachedCoverUrl(url);
  if (cached) return Promise.resolve(cached);

  const failedUntil = failures.get(url);
  if (failedUntil && failedUntil > Date.now()) return Promise.reject(new UnavailableCover(url));
  failures.delete(url);

  const pending = pendingRequests.get(url);
  if (pending) return pending;

  const request = loadCover(url).finally(() => {
    pendingRequests.delete(url);
  });
  pendingRequests.set(url, request);
  return request;
}
