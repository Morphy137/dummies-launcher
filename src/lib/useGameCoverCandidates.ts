import { useEffect, useState } from "react";
import type { Game } from "../types";
import { cachedArtwork, lookupArtwork } from "./artworkLookupCache";
import { resolveSteamCover } from "./backend";
import { getGameCoverCandidates } from "./covers";

const queue: Array<() => void> = [];
let active = 0;

function lookup(name: string, steamAppId: string | null | undefined): Promise<string[]> {
  return lookupArtwork(
    "cover",
    name,
    steamAppId,
    () =>
      new Promise<string[]>((resolve, reject) => {
        queue.push(() => {
          active += 1;
          void resolveSteamCover(name, steamAppId)
            .then(resolve, reject)
            .finally(() => {
              active -= 1;
              queue.shift()?.();
            });
        });
        if (active < 4) queue.shift()?.();
      }),
  );
}

export function useGameCoverCandidates(game: Game): string[] {
  const { name, steamAppId } = game;
  const key = JSON.stringify([name, steamAppId]);
  const [resolved, setResolved] = useState<{ key: string; urls: string[] }>(() => ({
    key,
    urls: cachedArtwork("cover", name, steamAppId),
  }));
  useEffect(() => {
    let cancelled = false;
    void lookup(name, steamAppId).then((urls) => {
      if (!cancelled) setResolved({ key, urls });
    });
    return () => {
      cancelled = true;
    };
  }, [key, name, steamAppId]);
  return [
    ...new Set([
      ...(resolved.key === key ? resolved.urls : cachedArtwork("cover", name, steamAppId)),
      ...getGameCoverCandidates(game),
    ]),
  ];
}
