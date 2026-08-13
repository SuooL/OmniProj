// The single home for OmniProj's global keyboard shortcuts. Keeping them in one hook means
// there is exactly one keydown listener and one place that decides what a chord does.
//
// Design rules from the UX contract:
//   - Cmd/Ctrl+F focus the local filter, Cmd/Ctrl+N open Add Project, Cmd/Ctrl+R pull-refresh.
//   - Those three modified chords fire even while a text input is focused.
//   - Cmd/Ctrl+R only prevents the browser's default reload while the OmniProj window is
//     focused (so a background window never eats a real reload).
//   - Only *unmodified* typing keys are left to text controls; a bare letter is never a
//     shortcut. Escape is a control key (not typing) and always reaches us so it can close
//     the topmost surface even from within an input.

import { useEffect, useRef } from "react";

export interface AppShortcutHandlers {
  onFocusFilter: () => void;
  onOpenAddProject: () => void;
  onRefresh: () => void;
  onEscape: () => void;
  /** Overridable window-focus probe; defaults to `document.hasFocus()`. */
  isWindowFocused?: () => boolean;
}

export function useAppShortcuts(handlers: AppShortcutHandlers): void {
  // Hold the latest handlers in a ref so the listener binds once and never goes stale.
  const ref = useRef(handlers);
  ref.current = handlers;

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent): void {
      const h = ref.current;

      if (event.key === "Escape") {
        h.onEscape();
        return;
      }

      const modified = event.metaKey || event.ctrlKey;
      if (!modified) return; // Unmodified typing belongs to whatever control has focus.

      switch (event.key.toLowerCase()) {
        case "f":
          event.preventDefault();
          h.onFocusFilter();
          break;
        case "n":
          event.preventDefault();
          h.onOpenAddProject();
          break;
        case "r": {
          const focused = (h.isWindowFocused ?? (() => document.hasFocus()))();
          if (focused) {
            event.preventDefault();
            h.onRefresh();
          }
          break;
        }
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
