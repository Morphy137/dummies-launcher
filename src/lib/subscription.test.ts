import { describe, expect, it, vi } from "vitest";
import { disposeSubscription } from "./subscription";

describe("subscription cleanup", () => {
  it("unregisters even when unmounted before registration resolves", async () => {
    let finish: (fn: () => void) => void = () => {};
    const registration = new Promise<() => void>((resolve) => {
      finish = resolve;
    });
    const unlisten = vi.fn();
    disposeSubscription(registration)();
    finish(unlisten);
    await registration;
    await Promise.resolve();
    expect(unlisten).toHaveBeenCalledOnce();
  });
});
