import { useEffect, useState } from "react";
import { getCachedCoverUrl, getCoverUrl, retainCover } from "./coverCache";

interface ResolvedCover {
  url: string;
  src?: string;
  failed: boolean;
}

export function useCachedCover(url: string | undefined): { src?: string; failed: boolean } {
  const [resolved, setResolved] = useState<ResolvedCover | null>(() => {
    if (!url) return null;
    const cached = getCachedCoverUrl(url);
    return cached ? { url, src: cached, failed: false } : null;
  });

  useEffect(() => {
    if (!url) {
      setResolved(null);
      return;
    }

    const release = retainCover(url);
    const cached = getCachedCoverUrl(url);
    if (cached) {
      setResolved({ url, src: cached, failed: false });
      return release;
    }

    let cancelled = false;
    setResolved(null);
    void getCoverUrl(url)
      .then((src) => {
        if (!cancelled) setResolved({ url, src, failed: false });
      })
      .catch(() => {
        if (!cancelled) setResolved({ url, failed: true });
      });

    return () => {
      cancelled = true;
      release();
    };
  }, [url]);

  if (!url) return { failed: false };
  const cached = getCachedCoverUrl(url);
  if (cached) return { src: cached, failed: false };
  return resolved?.url === url ? resolved : { failed: false };
}
