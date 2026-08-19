/**
 * タグ一覧のストア（`clipboardStore.ts`と同じパターン、Phase4 F-17）。
 */
import { writable } from "svelte/store";
import type { Tag } from "../types/tags";
import { tagsList } from "../api/commands";

function createTagsStore() {
  const { subscribe, set } = writable<Tag[]>([]);

  async function refresh(): Promise<void> {
    const tags = await tagsList();
    set(tags);
  }

  return { subscribe, refresh };
}

export const tagsStore = createTagsStore();
