// A small reactive media-query hook. Used to decide the Peek vs full-page viewport boundary
// (>=800px) without coupling routing to CSS. Guards against environments without matchMedia.

import { useEffect, useState } from "react";

export function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState<boolean>(() => {
    if (typeof window === "undefined" || !window.matchMedia) return false;
    return window.matchMedia(query).matches;
  });

  useEffect(() => {
    if (typeof window === "undefined" || !window.matchMedia) return;
    const list = window.matchMedia(query);
    const onChange = () => setMatches(list.matches);
    onChange();
    list.addEventListener("change", onChange);
    return () => list.removeEventListener("change", onChange);
  }, [query]);

  return matches;
}

/** True when the viewport is wide enough for the non-modal Peek inspector (spec: >=800px). */
export function useIsPeekViewport(): boolean {
  return useMediaQuery("(min-width: 800px)");
}
