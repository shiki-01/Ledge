/**
 * シェルフ内アイテム一覧のストア。
 *
 * Svelte 5のrunes（$state等）はコンポーネント（.svelte）または`.svelte.ts`拡張子のファイルでのみ
 * コンパイラに認識されるため、通常の`.ts`モジュールであるこのファイルでは
 * 従来どおり`svelte/store`の`writable`を使う（CLAUDE.mdが禁止する「旧構文」は
 * `export let` / `$:`によるコンポーネントのリアクティビティ記法を指しており、
 * `svelte/store`自体の利用を妨げるものではないと解釈した。コンポーネント内のローカル状態は
 * `$state`/`$derived`を使う）。
 */
import { writable } from "svelte/store";
import type { ShelfItem } from "../types/shelf";
import { shelfListItems } from "../api/commands";

function createShelfStore() {
  const { subscribe, set } = writable<ShelfItem[]>([]);

  async function refresh(): Promise<void> {
    const items = await shelfListItems();
    set(items);
  }

  return { subscribe, refresh };
}

export const shelfStore = createShelfStore();
