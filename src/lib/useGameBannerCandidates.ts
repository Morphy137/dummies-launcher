import { useEffect, useState } from "react";
import type { Game } from "../types";
import { cachedArtwork, lookupArtwork } from "./artworkLookupCache";
import { resolveSteamArtwork } from "./backend";
import { getGameBannerCandidates } from "./covers";

export function useGameBannerCandidates(game: Game): string[] {
  const { name, steamAppId } = game;
  const key = JSON.stringify([name, steamAppId]);
  const [resolved, setResolved] = useState<{ key: string; urls: string[] }>(() => ({
    key,
    urls: cachedArtwork("banner", name, steamAppId),
  }));
  useEffect(() => {
    let cancelled = false;
    void lookupArtwork("banner", name, steamAppId, () => resolveSteamArtwork(name, steamAppId))
      .then((urls) => {
        if (!cancelled) setResolved({ key, urls });
      })
      .catch(() => {
        // Existing artwork remains available when the Steam store is unreachable.
      });
    return () => {
      cancelled = true;
    };
  }, [key, name, steamAppId]);
  return [
    ...new Set([
      ...(resolved.key === key ? resolved.urls : cachedArtwork("banner", name, steamAppId)),
      ...getGameBannerCandidates(game),
    ]),
  ];
}
