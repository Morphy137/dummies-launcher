import { isTauri } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

export interface UpdateProgress {
  percent?: number;
}

export async function checkForUpdate(): Promise<Update | null> {
  if (!isTauri()) return null;
  return check();
}

export async function installUpdate(
  update: Update,
  onProgress: (progress: UpdateProgress) => void,
): Promise<void> {
  let total = 0;
  let downloaded = 0;

  await update.downloadAndInstall((event) => {
    if (event.event === "Started") {
      total = event.data.contentLength ?? 0;
      onProgress({ percent: 0 });
    } else if (event.event === "Progress") {
      downloaded += event.data.chunkLength;
      onProgress({
        percent: total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : undefined,
      });
    } else if (event.event === "Finished") {
      onProgress({ percent: 100 });
    }
  });

  await relaunch();
}
