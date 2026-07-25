import { getCurrentWindow } from "@tauri-apps/api/window";

export function minimizeAppWindow(): Promise<void> {
  return getCurrentWindow().minimize();
}

export function toggleMaximizeAppWindow(): Promise<void> {
  return getCurrentWindow().toggleMaximize();
}

export function closeAppWindow(): Promise<void> {
  return getCurrentWindow().close();
}
