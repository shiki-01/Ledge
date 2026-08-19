/**
 * アプリ設定のストア（shelfStore.tsと同様の理由でsvelte/storeを使う）。
 * Phase1では起動時の初期表示に使う程度で、設定変更UI自体はPhase3で追加する。
 */
import { writable } from "svelte/store";
import type { AppSettings } from "../types/settings";
import { getSettings } from "../api/commands";

function createSettingsStore() {
  const { subscribe, set } = writable<AppSettings | null>(null);

  async function refresh(): Promise<void> {
    const settings = await getSettings();
    set(settings);
  }

  return { subscribe, refresh };
}

export const settingsStore = createSettingsStore();
