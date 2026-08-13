// The only place the app touches the native directory picker. Isolating it keeps the Tauri
// plugin out of components and lets tests stub one seam. `open` with `directory: true` returns
// a string, an array, or null; we defensively collapse anything but a single string to null.

import { open } from "@tauri-apps/plugin-dialog";

export async function chooseProjectDirectory(): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false });
  return typeof selected === "string" ? selected : null;
}

/** The trailing path segment, used as the default project name. */
export function basename(path: string): string {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1] : path;
}
