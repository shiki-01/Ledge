/**
 * よく使うフォルダ一覧のストア（Phase6, F-09）。
 *
 * `shelfStore.ts`と全く同じパターン（`writable` + `refresh()`）にしている。runesが使えない理由は
 * shelfStore.tsのコメント参照。
 */
import { writable } from "svelte/store";
import type { FavoriteFolder } from "../types/favorites";
import { favoritesList } from "../api/commands";

function createFavoritesStore() {
  const { subscribe, set } = writable<FavoriteFolder[]>([]);

  async function refresh(): Promise<void> {
    const items = await favoritesList();
    set(items);
  }

  return { subscribe, refresh };
}

export const favoritesStore = createFavoritesStore();
