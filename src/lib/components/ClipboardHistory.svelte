<script lang="ts">
  import ClipboardItem from "./ClipboardItem.svelte";
  import { clipboardStore } from "../stores/clipboardStore";
  import { clipboardClear, clipboardDelete, clipboardPasteToActive, clipboardSetPinned } from "../api/commands";
  import { isShelfErrorPayload } from "../types/error";

  let errorMessage = $state<string | null>(null);
  let errorTimer: ReturnType<typeof setTimeout> | undefined;

  function showError(e: unknown): void {
    errorMessage = isShelfErrorPayload(e) ? e.message : "予期しないエラーが発生しました";
    clearTimeout(errorTimer);
    errorTimer = setTimeout(() => {
      errorMessage = null;
    }, 4000);
  }

  async function handlePaste(id: number): Promise<void> {
    try {
      await clipboardPasteToActive(id);
    } catch (e) {
      showError(e);
    }
  }

  async function handleTogglePin(id: number, pinned: boolean): Promise<void> {
    try {
      await clipboardSetPinned(id, !pinned);
    } catch (e) {
      showError(e);
    }
  }

  async function handleDelete(id: number): Promise<void> {
    try {
      await clipboardDelete(id);
    } catch (e) {
      showError(e);
    }
  }

  async function handleClearAll(): Promise<void> {
    try {
      // ピン留めアイテムは一括削除の対象から除外する（F-13）
      await clipboardClear(true);
    } catch (e) {
      showError(e);
    }
  }
</script>

<div class="clipboard-history">
  <header class="clipboard-history__header">
    <span class="clipboard-history__title">クリップボード履歴</span>
    <button
      type="button"
      class="clipboard-history__clear"
      disabled={$clipboardStore.length === 0}
      onclick={handleClearAll}
    >
      全て削除
    </button>
  </header>

  {#if errorMessage}
    <div class="clipboard-history__toast" role="alert">{errorMessage}</div>
  {/if}

  <ul class="clipboard-history__list">
    {#each $clipboardStore as entry (entry.id)}
      <li>
        <ClipboardItem
          {entry}
          onPaste={() => handlePaste(entry.id)}
          onTogglePin={() => handleTogglePin(entry.id, entry.pinned)}
          onDelete={() => handleDelete(entry.id)}
        />
      </li>
    {/each}
  </ul>

  {#if $clipboardStore.length === 0}
    <p class="clipboard-history__empty">コピーした内容がここに表示されます</p>
  {/if}
</div>

<style>
  .clipboard-history {
    display: flex;
    flex-direction: column;
    height: 100%;
    box-sizing: border-box;
    padding: 0.6rem;
    gap: 0.5rem;
  }

  .clipboard-history__header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .clipboard-history__title {
    color: #f5f5f5;
    font-size: 0.85rem;
    font-weight: 600;
  }

  .clipboard-history__clear {
    background: rgba(255, 255, 255, 0.1);
    border: none;
    color: #f5f5f5;
    border-radius: 4px;
    padding: 0.2rem 0.5rem;
    font-size: 0.7rem;
    cursor: pointer;
  }

  .clipboard-history__clear:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .clipboard-history__toast {
    background: rgba(220, 38, 38, 0.85);
    color: #fff;
    font-size: 0.75rem;
    padding: 0.4rem 0.6rem;
    border-radius: 4px;
  }

  .clipboard-history__list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    overflow-y: auto;
  }

  .clipboard-history__empty {
    color: rgba(245, 245, 245, 0.6);
    font-size: 0.75rem;
    text-align: center;
    margin-top: 1rem;
  }
</style>
