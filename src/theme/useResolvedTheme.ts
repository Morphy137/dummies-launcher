import { useEffect, useState } from "react";
import type { ThemeMode } from "../types";

export type ResolvedTheme = "dark" | "light";

function readSystemTheme(): ResolvedTheme {
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function useResolvedTheme(preference: ThemeMode): ResolvedTheme {
  const [systemTheme, setSystemTheme] = useState<ResolvedTheme>(readSystemTheme);

  useEffect(() => {
    const query = window.matchMedia("(prefers-color-scheme: dark)");
    const sync = () => setSystemTheme(query.matches ? "dark" : "light");

    sync();
    query.addEventListener("change", sync);
    return () => query.removeEventListener("change", sync);
  }, []);

  const resolved = preference === "system" ? systemTheme : preference;

  useEffect(() => {
    document.documentElement.dataset.theme = resolved;
  }, [resolved]);

  return resolved;
}
