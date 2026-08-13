// Vitest global setup: jest-dom matchers, DOM cleanup, and a controllable matchMedia (jsdom
// ships none). Tests default to a wide (Peek) viewport; set `mediaState.matches = false` before
// rendering to simulate < 800px.
import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

export const mediaState = { matches: true };

Object.defineProperty(window, "matchMedia", {
  writable: true,
  configurable: true,
  value: (query: string): MediaQueryList =>
    ({
      matches: mediaState.matches,
      media: query,
      onchange: null,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
    }) as unknown as MediaQueryList,
});

afterEach(() => {
  mediaState.matches = true;
  cleanup();
});
