import type { Game } from "../types";

export function getGameInitials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return "??";
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
  return `${parts[0][0]}${parts[1][0]}`.toUpperCase();
}

export function getGameCoverCandidates(game: Game): string[] {
  const candidates: string[] = [];
  const steamId = String(game.steamAppId || "").trim();

  if (/^\d+$/.test(steamId)) {
    candidates.push(
      `https://shared.fastly.steamstatic.com/store_item_assets/steam/apps/${steamId}/library_600x900_2x.jpg`,
    );
    candidates.push(
      `https://shared.fastly.steamstatic.com/store_item_assets/steam/apps/${steamId}/library_600x900.jpg`,
    );
    candidates.push(`https://cdn.akamai.steamstatic.com/steam/apps/${steamId}/header.jpg`);
  }

  const appId = String(game.appId || "").trim();
  if (appId && game.coverImageHash) {
    candidates.push(
      `https://cdn.discordapp.com/app-icons/${appId}/${game.coverImageHash}.png?size=512`,
    );
  }
  if (appId && game.iconHash) {
    candidates.push(`https://cdn.discordapp.com/app-icons/${appId}/${game.iconHash}.png?size=256`);
  }

  return candidates;
}

export function getGameBannerCandidates(game: Game): string[] {
  const candidates: string[] = [];
  const steamId = String(game.steamAppId || "").trim();
  if (/^\d+$/.test(steamId)) {
    candidates.push(
      `https://shared.fastly.steamstatic.com/store_item_assets/steam/apps/${steamId}/library_hero_2x.jpg`,
      `https://shared.fastly.steamstatic.com/store_item_assets/steam/apps/${steamId}/library_hero.jpg`,
      `https://cdn.akamai.steamstatic.com/steam/apps/${steamId}/header.jpg`,
    );
  }

  return candidates;
}

const fallbackPalettes: Array<[string, string]> = [
  ["#52603b", "#111a0d"],
  ["#6b4a2a", "#17120c"],
  ["#6c2c25", "#1b0d0b"],
  ["#2b6b7a", "#0c1c24"],
  ["#6a6d72", "#15171a"],
  ["#5a1b1b", "#130808"],
  ["#c26b22", "#2a1206"],
  ["#3f4f5c", "#0f161c"],
  ["#2e6b4e", "#0b1a13"],
  ["#3a6b76", "#0c1a20"],
  ["#6c2d2d", "#1b0b0b"],
  ["#3d4f8c", "#0d1124"],
];

export function getFallbackArt(name: string): [string, string] {
  let hash = 0;
  for (let index = 0; index < name.length; index += 1) {
    hash = (hash * 31 + name.charCodeAt(index)) >>> 0;
  }
  return fallbackPalettes[hash % fallbackPalettes.length];
}
