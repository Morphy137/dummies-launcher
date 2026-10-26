import { afterEach, describe, expect, it, vi } from "vitest";

function imageResponse() {
  return new Response(new Blob(["cover"], { type: "image/png" }));
}

function mockStorage() {
  const entries = new Map<string, Response>();
  const cache = {
    match: vi.fn(async (url: string) => entries.get(url)?.clone()),
    put: vi.fn(async (url: string, response: Response) => {
      entries.set(url, response.clone());
    }),
    delete: vi.fn(async (key: string | Request) =>
      entries.delete(typeof key === "string" ? key : key.url),
    ),
    keys: vi.fn(async () => [...entries.keys()].map((url) => new Request(url))),
  };
  vi.stubGlobal("caches", { open: vi.fn().mockResolvedValue(cache) });
  return { cache, entries };
}

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.resetModules();
});

describe("cover cache", () => {
  it("deduplicates pending downloads and keeps the same object URL across filter changes", async () => {
    vi.useFakeTimers();
    const { cache } = mockStorage();
    const fetchMock = vi.fn(async () => imageResponse());
    vi.stubGlobal("fetch", fetchMock);
    const create = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:cover");
    const revoke = vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
    const { getCoverUrl, retainCover } = await import("./coverCache");
    const url = "https://example.test/cover.png";
    const release = retainCover(url);
    await expect(Promise.all([getCoverUrl(url), getCoverUrl(url)])).resolves.toEqual([
      "blob:cover",
      "blob:cover",
    ]);
    release();
    await vi.advanceTimersByTimeAsync(5 * 60_000);
    const releaseAgain = retainCover(url);
    await expect(getCoverUrl(url)).resolves.toBe("blob:cover");
    expect(fetchMock).toHaveBeenCalledOnce();
    expect(cache.put).toHaveBeenCalledOnce();
    expect(create).toHaveBeenCalledOnce();
    expect(revoke).not.toHaveBeenCalled();
    releaseAgain();
  });

  it.each(["missing", "denied"])("still caches in memory when disk storage is %s", async (mode) => {
    vi.stubGlobal(
      "caches",
      mode === "missing" ? undefined : { open: vi.fn().mockRejectedValue(new Error("denied")) },
    );
    const fetchMock = vi.fn(async () => imageResponse());
    vi.stubGlobal("fetch", fetchMock);
    vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:memory");
    const { getCoverUrl } = await import("./coverCache");
    await expect(getCoverUrl("https://example.test/memory.png")).resolves.toBe("blob:memory");
    await expect(getCoverUrl("https://example.test/memory.png")).resolves.toBe("blob:memory");
    expect(fetchMock).toHaveBeenCalledOnce();
  });

  it("reads persistent images after memory is cleared without downloading again", async () => {
    mockStorage();
    const fetchMock = vi.fn(async () => imageResponse());
    vi.stubGlobal("fetch", fetchMock);
    vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:disk");
    const first = await import("./coverCache");
    await first.getCoverUrl("https://example.test/disk.png");
    vi.resetModules();
    const next = await import("./coverCache");
    await expect(next.getCoverUrl("https://example.test/disk.png")).resolves.toBe("blob:disk");
    expect(fetchMock).toHaveBeenCalledOnce();
  });

  it("expires persistent images after seven days", async () => {
    const { entries } = mockStorage();
    entries.set(
      "https://example.test/old.png",
      new Response("old", {
        headers: {
          "content-type": "image/png",
          "x-dummies-cached-at": String(Date.now() - 8 * 24 * 60 * 60_000),
        },
      }),
    );
    const fetchMock = vi.fn(async () => imageResponse());
    vi.stubGlobal("fetch", fetchMock);
    vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:fresh");
    const { getCoverUrl } = await import("./coverCache");
    await expect(getCoverUrl("https://example.test/old.png")).resolves.toBe("blob:fresh");
    expect(fetchMock).toHaveBeenCalledOnce();
  });

  it("evicts least-recently-used idle images without revoking a mounted image", async () => {
    vi.stubGlobal("caches", undefined);
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => imageResponse()),
    );
    let next = 0;
    vi.spyOn(URL, "createObjectURL").mockImplementation(() => `blob:${next++}`);
    const revoke = vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
    const { getCoverUrl, getCachedCoverUrl, retainCover } = await import("./coverCache");
    const active = "https://example.test/0.png";
    const release = retainCover(active);
    for (let index = 0; index < 129; index++)
      await getCoverUrl(`https://example.test/${index}.png`);
    expect(getCachedCoverUrl(active)).toBe("blob:0");
    expect(getCachedCoverUrl("https://example.test/1.png")).toBeUndefined();
    expect(revoke).toHaveBeenCalledExactlyOnceWith("blob:1");
    release();
  });

  it("does not request known missing images again until their cooldown expires", async () => {
    vi.useFakeTimers();
    vi.stubGlobal("caches", undefined);
    const fetchMock = vi.fn(async () => new Response(null, { status: 404 }));
    vi.stubGlobal("fetch", fetchMock);
    const { getCoverUrl } = await import("./coverCache");
    const url = "https://example.test/missing.png";
    await expect(getCoverUrl(url)).rejects.toThrow();
    await expect(getCoverUrl(url)).rejects.toThrow();
    expect(fetchMock).toHaveBeenCalledOnce();
    await vi.advanceTimersByTimeAsync(600_001);
    await expect(getCoverUrl(url)).rejects.toThrow();
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("bounds idle image bytes as well as the number of entries", async () => {
    vi.stubGlobal("caches", undefined);
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        const response = imageResponse();
        vi.spyOn(response, "blob").mockResolvedValue({ size: 20 * 1024 * 1024 } as Blob);
        return response;
      }),
    );
    let next = 0;
    vi.spyOn(URL, "createObjectURL").mockImplementation(() => `blob:large-${next++}`);
    const revoke = vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
    const { getCoverUrl, getCachedCoverUrl } = await import("./coverCache");
    await getCoverUrl("https://example.test/large-a.png");
    await getCoverUrl("https://example.test/large-b.png");
    expect(revoke).toHaveBeenCalledExactlyOnceWith("blob:large-0");
    expect(getCachedCoverUrl("https://example.test/large-a.png")).toBeUndefined();
    expect(getCachedCoverUrl("https://example.test/large-b.png")).toBe("blob:large-1");
  });

  it("keeps the direct image fallback for hosts that reject CORS fetches", async () => {
    vi.stubGlobal("caches", undefined);
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("CORS")));
    const { getCoverUrl } = await import("./coverCache");
    await expect(getCoverUrl("https://example.test/cors.png")).resolves.toBe(
      "https://example.test/cors.png",
    );
  });
});
