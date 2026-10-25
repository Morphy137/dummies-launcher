import { afterEach, describe, expect, it, vi } from "vitest";

describe("cover cache", () => {
  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    vi.resetModules();
  });

  it("fetches a cover once and reuses its object URL", async () => {
    vi.useFakeTimers();
    const blob = new Blob(["cover"], { type: "image/png" });
    const response = {
      ok: true,
      status: 200,
      headers: new Headers({ "content-type": "image/png" }),
      clone: () => response,
      blob: async () => blob,
    } as unknown as Response;
    const cache = {
      match: vi.fn().mockResolvedValue(undefined),
      put: vi.fn().mockResolvedValue(undefined),
      keys: vi.fn().mockResolvedValue([]),
    };
    vi.stubGlobal("caches", { open: vi.fn().mockResolvedValue(cache) });
    const fetchMock = vi.fn().mockResolvedValue(response);
    vi.stubGlobal("fetch", fetchMock);
    const createObjectURL = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:cover");

    const { getCoverUrl } = await import("./coverCache");
    const url = "https://example.test/cover.png";

    await expect(getCoverUrl(url)).resolves.toBe("blob:cover");
    await expect(getCoverUrl(url)).resolves.toBe("blob:cover");

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(cache.put).toHaveBeenCalledTimes(1);
    expect(createObjectURL).toHaveBeenCalledTimes(1);
  });

  it("falls back to the remote URL when storage is unavailable", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    vi.stubGlobal("caches", undefined);

    const { getCoverUrl } = await import("./coverCache");

    await expect(getCoverUrl("https://example.test/cover.png")).resolves.toBe(
      "https://example.test/cover.png",
    );
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("revokes inactive object URLs but keeps covers still used by another component", async () => {
    vi.useFakeTimers();
    const response = new Response(new Blob(["cover"], { type: "image/png" }));
    vi.stubGlobal("caches", {
      open: vi.fn().mockResolvedValue({
        match: vi.fn().mockResolvedValue(undefined),
        put: vi.fn(),
        keys: vi.fn().mockResolvedValue([]),
      }),
    });
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(response));
    vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:shared");
    const revoke = vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
    const { retainCover, getCoverUrl } = await import("./coverCache");
    const releaseA = retainCover("https://example.test/shared");
    const releaseB = retainCover("https://example.test/shared");
    await getCoverUrl("https://example.test/shared");
    releaseA();
    await vi.advanceTimersByTimeAsync(30_001);
    expect(revoke).not.toHaveBeenCalled();
    releaseB();
    await vi.advanceTimersByTimeAsync(30_001);
    expect(revoke).toHaveBeenCalledWith("blob:shared");
  });
});
