// Vitest global setup: jest-dom matchers, DOM cleanup, and a controllable matchMedia (jsdom
// ships none). Tests default to a wide viewport; set `mediaState.matches = false` before
// rendering to simulate < 800px.
import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

export const mediaState = { matches: true };

// Node may expose an unavailable experimental localStorage that shadows jsdom's storage.
// Install a deterministic per-worker implementation so restart-restoration tests exercise the
// same Web Storage contract as the desktop webview.
const localValues = new Map<string, string>();
const testLocalStorage: Storage = {
  get length() { return localValues.size; },
  clear: () => localValues.clear(),
  getItem: (key) => localValues.get(key) ?? null,
  key: (index) => Array.from(localValues.keys())[index] ?? null,
  removeItem: (key) => { localValues.delete(key); },
  setItem: (key, value) => { localValues.set(key, String(value)); },
};
Object.defineProperty(window, "localStorage", {
  configurable: true,
  value: testLocalStorage,
});

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
