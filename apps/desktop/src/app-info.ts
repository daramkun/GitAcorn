import { invoke } from "@tauri-apps/api/core";

export type AppInfoDto = {
  schemaVersion: 1;
  name: string;
  version: string;
  runtime: string;
};

export function getAppInfo(): Promise<AppInfoDto> {
  return invoke<AppInfoDto>("app_info");
}
