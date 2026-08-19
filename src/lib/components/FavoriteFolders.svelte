<script lang="ts">
  /**
   * よく使うフォルダ（Phase6, F-09）。シェルフ一覧の上に常時表示するセクション。
   * `shelf_items`とはライフサイクルが異なる別テーブル（architecture.md 12.1章）のため、
   * 通常のシェルフ一覧・「全て削除」ロジックとは独立させている。
   */
  import { open } from "@tauri-apps/plugin-dialog";
  import { favoritesStore } from "../stores/favoritesStore";
  import { favoritesAdd, favoritesBeginDragOut, favoritesRemove } from "../api/commands";

  interface Props {
    /** エラー表示はShelf.svelte側のトーストへ集約する（ShelfItemのonErrorと同じコールバックpropsパターン） */
    onError: (e: unknown) => void;
  }

  let { onError }: Props = $props();

  async function handleAddClick(): Promise<void> {
    try {
      const selected = await open({ directory: true });
      // ダイアログをキャンセルした場合はnullが返る
      if (typeof selected !== "string") {
        return;
      }
      await favoritesAdd(selected);
    } catch (e) {
      onError(e);
    }
  }

  async function handleDragOut(id: number): Promise<void> {
    try {
      await favoritesBeginDragOut(id);
    } catch (e) {
      onError(e);
    }
  }

  async function handleRemove(id: number): Promise<void> {
    try {
      await favoritesRemove(id);
    } catch (e) {
      onError(e);
    }
  }
</script>

<div class="favorites">
  <div class="favorites__header">
    <span class="favorites__title">よく使うフォルダ</span>
    <button type="button" class="favorites__add" onclick={handleAddClick} aria-label="フォルダを追加">
      ＋
    </button>
  </div>

  {#if $favoritesStore.length > 0}
    <ul class="favorites__list">
      {#each $favoritesStore as folder (folder.id)}
        <li>
          <div
            class="favorites__item"
            class:favorites__item--missing={folder.missing}
            title={folder.folderPath}
          >
            <button
              type="button"
              class="favorites__body"
              onmousedown={() => handleDragOut(folder.id)}
              disabled={folder.missing}
            >
              <span class="favorites__name">{folder.displayName}</span>
              {#if folder.missing}
                <span class="favorites__warning">見つかりません</span>
              {/if}
            </button>
            <button
              type="button"
              class="favorites__remove"
              onclick={() => handleRemove(folder.id)}
              aria-label="よく使うフォルダから削除"
            >
              ×
            </button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .favorites {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    padding-bottom: 0.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .favorites__header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .favorites__title {
    color: rgba(245, 245, 245, 0.7);
    font-size: 0.7rem;
    font-weight: 600;
  }

  .favorites__add {
    background: rgba(255, 255, 255, 0.1);
    border: none;
    color: #f5f5f5;
    border-radius: 4px;
    padding: 0.1rem 0.45rem;
    font-size: 0.75rem;
    line-height: 1.4;
    cursor: pointer;
  }

  .favorites__list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    max-height: 30vh;
    overflow-y: auto;
  }

  .favorites__item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.3rem 0.6rem;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.05);
    color: #f5f5f5;
  }

  .favorites__item--missing {
    opacity: 0.5;
  }

  .favorites__body {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    background: none;
    border: none;
    color: inherit;
    text-align: left;
    cursor: grab;
    padding: 0;
    font: inherit;
  }

  .favorites__body:disabled {
    cursor: not-allowed;
  }

  .favorites__name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.8rem;
  }

  .favorites__warning {
    flex-shrink: 0;
    font-size: 0.65rem;
    color: #ff9d9d;
  }

  .favorites__remove {
    flex-shrink: 0;
    background: none;
    border: none;
    color: inherit;
    opacity: 0.6;
    cursor: pointer;
    font-size: 0.85rem;
    line-height: 1;
    padding: 0.2rem 0.4rem;
  }

  .favorites__remove:hover {
    opacity: 1;
  }
</style>
