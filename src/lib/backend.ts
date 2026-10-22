import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  type Bootstrap,
  DEFAULT_SETTINGS,
  type DummyExitedPayload,
  type LaunchRequest,
  type LaunchResult,
  type Settings,
} from "../types";

function browserBootstrap(): Bootstrap {
  return {
    settings: DEFAULT_SETTINGS,
    systemLocale: navigator.language || "en",
    runningKeys: [],
    version: "dev",
  };
}

export async function getBootstrap(): Promise<Bootstrap> {
  if (!isTauri()) return browserBootstrap();
  return invoke<Bootstrap>("get_bootstrap");
}

export async function saveSettings(settings: Settings): Promise<Settings> {
  if (!isTauri()) return settings;
  return invoke<Settings>("save_settings", { settings });
}

export async function launchDummy(request: LaunchRequest): Promise<LaunchResult> {
  if (!isTauri()) {
    throw new Error("The desktop backend is not available in the browser preview.");
  }
  return invoke<LaunchResult>("launch_dummy", { request });
}

export async function stopDummy(gameKey: string): Promise<boolean> {
  if (!isTauri()) return false;
  return invoke<boolean>("stop_dummy", { gameKey });
}

export async function stopAllDummies(): Promise<number> {
  if (!isTauri()) return 0;
  return invoke<number>("stop_all_dummies");
}

export async function listRunningDummies(): Promise<string[]> {
  if (!isTauri()) return [];
  return invoke<string[]>("list_running_dummies");
}

export function onDummyExited(handler: (payload: DummyExitedPayload) => void): () => void {
  if (!isTauri()) return () => {};

  let unlisten: (() => void) | undefined;
  void listen<DummyExitedPayload>("dummy://exited", (event) => handler(event.payload)).then(
    (fn) => {
      unlisten = fn;
    },
  );

  return () => {
    unlisten?.();
  };
}
