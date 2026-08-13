// The application root: it owns the BrowserRouter and nothing else. Canonical routes, the
// interaction frame and route surfaces live in AppShell. Before the router reads
// window.location, we restore the last canonical route — but only when the incoming path is
// "/", so an explicit deep link always wins.

import { useState } from "react";
import { BrowserRouter } from "react-router-dom";

import { AppShell } from "./components/AppShell";
import { restoreCanonicalRouteOnRoot } from "./domain/navigationSession";

export function App() {
  // Run the pre-mount restore exactly once, synchronously, before BrowserRouter renders. The
  // restore is idempotent and a no-op for any non-root path.
  useState(() => {
    restoreCanonicalRouteOnRoot();
    return null;
  });

  return (
    <BrowserRouter>
      <AppShell />
    </BrowserRouter>
  );
}
