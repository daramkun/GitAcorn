import { invoke } from "@tauri-apps/api/core";

const iconCache = new Map<string, string>();

export async function getSystemFileIcons(
  worktreePath: string,
  paths: string[],
): Promise<Record<string, string>> {
  const result: Record<string, string> = {};
  const uncached = paths.filter((path) => {
    const key = `${worktreePath}\0${path}`;
    const cached = iconCache.get(key);
    if (cached) result[path] = cached;
    return !cached;
  });

  if (uncached.length === 0) return result;

  const loaded = await invoke<Record<string, string>>("system_file_icons", {
    worktreePath,
    paths: uncached,
  });
  for (const path of uncached) {
    const key = `${worktreePath}\0${path}`;
    const icon = loaded[path];
    if (icon) {
      iconCache.set(key, icon);
      result[path] = icon;
    }
  }
  return result;
}
