// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";

afterEach(() => {
  localStorage.clear();
  vi.useRealTimers();
  vi.resetModules();
});

it.each(["cover", "banner"] as const)(
  "persists %s URLs and reuses them after restart",
  async (kind) => {
    const first = await import("./artworkLookupCache");
    const resolve = vi.fn().mockResolvedValue(["https://example.test/image"]);
    await expect(first.lookupArtwork(kind, "Game", null, resolve)).resolves.toEqual([
      "https://example.test/image",
    ]);
    await first.lookupArtwork(kind, "Game", null, resolve);
    vi.resetModules();
    const next = await import("./artworkLookupCache");
    expect(next.cachedArtwork(kind, "Game", null)).toEqual(["https://example.test/image"]);
    await next.lookupArtwork(kind, "Game", null, resolve);
    expect(resolve).toHaveBeenCalledOnce();
  },
);

it("keeps expired artwork when refresh fails and retries later", async () => {
  vi.useFakeTimers();
  const cache = await import("./artworkLookupCache");
  const resolve = vi
    .fn()
    .mockResolvedValueOnce(["old-image"])
    .mockRejectedValueOnce(new Error("offline"))
    .mockResolvedValueOnce(["new-image"]);
  await cache.lookupArtwork("banner", "Game", null, resolve);
  await vi.advanceTimersByTimeAsync(8 * 24 * 60 * 60_000);
  await expect(cache.lookupArtwork("banner", "Game", null, resolve)).resolves.toEqual([
    "old-image",
  ]);
  await cache.lookupArtwork("banner", "Game", null, resolve);
  expect(resolve).toHaveBeenCalledTimes(2);
  await vi.advanceTimersByTimeAsync(5 * 60_000 + 1);
  await expect(cache.lookupArtwork("banner", "Game", null, resolve)).resolves.toEqual([
    "new-image",
  ]);
});

it("deduplicates concurrent lookups", async () => {
  const cache = await import("./artworkLookupCache");
  const resolve = vi.fn().mockResolvedValue(["image"]);
  await Promise.all([
    cache.lookupArtwork("cover", "Game", null, resolve),
    cache.lookupArtwork("cover", "Game", null, resolve),
  ]);
  expect(resolve).toHaveBeenCalledOnce();
});
