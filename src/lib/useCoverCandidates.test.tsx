// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getCachedCoverUrl, getCoverUrl, retainCover } from "./coverCache";
import { useCoverCandidates } from "./useCoverCandidates";

vi.mock("./coverCache", () => ({
  getCachedCoverUrl: vi.fn(),
  getCoverUrl: vi.fn(),
  retainCover: vi.fn(),
}));
beforeEach(() => {
  vi.mocked(getCachedCoverUrl).mockReset().mockReturnValue(undefined);
  vi.mocked(getCoverUrl).mockReset();
  vi.mocked(retainCover).mockReset().mockReturnValue(vi.fn());
});
afterEach(cleanup);

describe("cover candidate selection", () => {
  it("remembers the successful fallback instead of retrying missing artwork on every remount", async () => {
    vi.mocked(getCoverUrl).mockImplementation(async (url) => {
      if (url === "missing-cover") throw new Error("404");
      return "blob:working";
    });
    const first = renderHook(() => useCoverCandidates(["missing-cover", "working-cover"]));
    await waitFor(() => expect(first.result.current.src).toBe("blob:working"));
    act(() => first.result.current.handleLoad());
    first.unmount();
    vi.mocked(getCoverUrl).mockClear();
    const next = renderHook(() => useCoverCandidates(["missing-cover", "working-cover"]));
    await waitFor(() => expect(next.result.current.src).toBe("blob:working"));
    expect(getCoverUrl).toHaveBeenCalledExactlyOnceWith("working-cover");
  });

  it("does not inherit a previous game's fallback index or accept its stale response", async () => {
    let finish: (src: string) => void = () => {};
    vi.mocked(getCoverUrl).mockImplementation((url) =>
      url === "old-cover"
        ? new Promise((resolve) => {
            finish = resolve;
          })
        : Promise.resolve("blob:new"),
    );
    const hook = renderHook(({ urls }) => useCoverCandidates(urls), {
      initialProps: { urls: ["old-cover"] },
    });
    hook.rerender({ urls: ["new-cover"] });
    await waitFor(() => expect(hook.result.current.src).toBe("blob:new"));
    await act(async () => {
      finish("blob:old");
    });
    expect(hook.result.current.src).toBe("blob:new");
  });

  it("shows a fallback immediately when no artwork exists", () => {
    const hook = renderHook(() => useCoverCandidates([]));
    expect(hook.result.current.failed).toBe(true);
    expect(getCoverUrl).not.toHaveBeenCalled();
  });
});
