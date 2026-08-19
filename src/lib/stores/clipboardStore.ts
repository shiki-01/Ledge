/**
 * クリップボード履歴一覧のストア（`shelfStore.ts`と同じパターン）。
 * Svelte 5のrunesは`.svelte`/`.svelte.ts`でのみ有効なため、通常の`.ts`モジュールである
 * このファイルでは`svelte/store`の`writable`を使う。
 */
import { writable } from "svelte/store";
import type { ClipboardEntry } from "../types/clipboard";
import { clipboardListHistory } from "../api/commands";

function createClipboardStore() {
  const { subscribe, set } = writable<ClipboardEntry[]>([]);

  async function refresh(query?: string): Promise<void> {
    const entries = await clipboardListHistory(query);
    set(entries);
  }

  return { subscribe, refresh };
}

export const clipboardStore = createClipboardStore();
