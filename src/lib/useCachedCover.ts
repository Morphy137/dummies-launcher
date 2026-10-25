import { useEffect, useState } from "react";
import { getCachedCoverUrl, getCoverUrl, retainCover } from "./coverCache";

interface ResolvedCover {
  url: string;
  src: string;
}

export function useCachedCover(url: string | undefined): string | undefined {
  const [resolved, setResolved] = useState<ResolvedCover | null>(() => {
    if (!url) return null;
    const cached = getCachedCoverUrl(url);
    return cached ? { url, src: cached } : null;
  });

  useEffect(() => {
    if (!url) {
      setResolved(null);
      return;
    }

    const release = retainCover(url);
    const cached = getCachedCoverUrl(url);
    if (cached) {
      setResolved({ url, src: cached });
      return release;
    }

    let cancelled = false;
    setResolved(null);
    void getCoverUrl(url).then((src) => {
      if (!cancelled) {
        setResolved({ url, src });
      }
    });

    return () => {
      cancelled = true;
      release();
    };
  }, [url]);

  if (!url) return undefined;
  return getCachedCoverUrl(url) ?? (resolved?.url === url ? resolved.src : undefined);
}
