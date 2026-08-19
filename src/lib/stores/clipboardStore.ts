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

  // 検索語/タグ絞り込みを内部で保持しておく。`clipboard://history-changed`イベント経由の
  // 再取得（App.svelte）は引数無しで`refresh()`を呼ぶため、ここで覚えておかないとイベントの
  // たびに検索条件が消えてしまう（迷った設計判断: 呼び出し元へ報告）。
  let currentQuery: string | undefined;
  let currentTagId: number | undefined;

  async function refresh(): Promise<void> {
    const entries = await clipboardListHistory(currentQuery, currentTagId);
    set(entries);
  }

  /** 検索語/タグ絞り込みを変更してから再取得する（F-14, F-17）。 */
  async function setFilters(query?: string, tagId?: number): Promise<void> {
    currentQuery = query;
    currentTagId = tagId;
    await refresh();
  }

  return { subscribe, refresh, setFilters };
}

export const clipboardStore = createClipboardStore();
