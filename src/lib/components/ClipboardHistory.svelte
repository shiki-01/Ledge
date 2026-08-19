<script lang="ts">
  import ClipboardItem from "./ClipboardItem.svelte";
  import { clipboardStore } from "../stores/clipboardStore";
  import { tagsStore } from "../stores/tagsStore";
  import {
    clipboardClear,
    clipboardDelete,
    clipboardPasteToActive,
    clipboardSetPinned,
    clipboardSetTags,
    clipboardStackEntries,
    tagsCreate,
    tagsDelete,
  } from "../api/commands";
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

  // F-14: 検索。入力のたびに簡易デバウンス（300ms）してから再取得する
  let searchInput = $state("");
  let searchDebounceTimer: ReturnType<typeof setTimeout> | undefined;

  // F-17: タグでの絞り込み（nullは「すべて」）
  let selectedTagId = $state<number | null>(null);

  function scheduleSearch(): void {
    clearTimeout(searchDebounceTimer);
    searchDebounceTimer = setTimeout(() => {
      void applyFilters();
    }, 300);
  }

  async function applyFilters(): Promise<void> {
    try {
      await clipboardStore.setFilters(searchInput || undefined, selectedTagId ?? undefined);
    } catch (e) {
      showError(e);
    }
  }

  function handleTagFilterClick(tagId: number | null): void {
    selectedTagId = tagId;
    void applyFilters();
  }

  // F-15: 複数選択→スタック（結合）
  let selectMode = $state(false);
  let selectedIds = $state<Set<number>>(new Set());

  function toggleSelectMode(): void {
    selectMode = !selectMode;
    selectedIds = new Set();
  }

  function handleToggleSelect(id: number): void {
    const next = new Set(selectedIds);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    selectedIds = next;
  }

  async function handleStack(): Promise<void> {
    // 一覧の表示順（=$clipboardStoreの並び順）を維持してid配列を作る（architecture.md 9.2章）
    const orderedIds = $clipboardStore.filter((entry) => selectedIds.has(entry.id)).map((entry) => entry.id);
    if (orderedIds.length < 2) return;
    try {
      await clipboardStackEntries(orderedIds);
      selectMode = false;
      selectedIds = new Set();
    } catch (e) {
      showError(e);
    }
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

  // F-17: タグ付け・タグ管理
  async function handleCreateTag(name: string, color: string): Promise<void> {
    try {
      await tagsCreate(name, color);
    } catch (e) {
      showError(e);
    }
  }

  async function handleDeleteTag(tagId: number): Promise<void> {
    try {
      await tagsDelete(tagId);
      if (selectedTagId === tagId) {
        selectedTagId = null;
      }
    } catch (e) {
      showError(e);
    }
  }

  async function handleToggleTag(entryId: number, currentTagIds: number[], tagId: number, checked: boolean): Promise<void> {
    const nextTagIds = checked ? [...currentTagIds, tagId] : currentTagIds.filter((id) => id !== tagId);
    try {
      await clipboardSetTags(entryId, nextTagIds);
    } catch (e) {
      showError(e);
    }
  }
</script>

<div class="clipboard-history">
  <header class="clipboard-history__header">
    <span class="clipboard-history__title">クリップボード履歴</span>
    <div class="clipboard-history__header-actions">
      <button type="button" class="clipboard-history__stack-toggle" onclick={toggleSelectMode}>
        {selectMode ? "選択をやめる" : "選択して結合"}
      </button>
      <button
        type="button"
        class="clipboard-history__clear"
        disabled={$clipboardStore.length === 0}
        onclick={handleClearAll}
      >
        全て削除
      </button>
    </div>
  </header>

  <input
    class="clipboard-history__search"
    type="search"
    placeholder="履歴を検索（本文・ファイルパス・タグ名）"
    bind:value={searchInput}
    oninput={scheduleSearch}
  />

  <div class="clipboard-history__tag-filters">
    <button
      type="button"
      class="clipboard-history__tag-chip"
      class:clipboard-history__tag-chip--active={selectedTagId === null}
      onclick={() => handleTagFilterClick(null)}
    >
      すべて
    </button>
    {#each $tagsStore as tag (tag.id)}
      <button
        type="button"
        class="clipboard-history__tag-chip"
        class:clipboard-history__tag-chip--active={selectedTagId === tag.id}
        style={`--tag-color: ${tag.color ?? "#888"}`}
        onclick={() => handleTagFilterClick(tag.id)}
      >
        {tag.name}
      </button>
    {/each}
  </div>

  {#if selectMode}
    <div class="clipboard-history__stack-bar">
      <span>{selectedIds.size}件選択中（テキストのみ選択可）</span>
      <button type="button" disabled={selectedIds.size < 2} onclick={handleStack}>結合してテキスト化</button>
    </div>
  {/if}

  {#if errorMessage}
    <div class="clipboard-history__toast" role="alert">{errorMessage}</div>
  {/if}

  <ul class="clipboard-history__list">
    {#each $clipboardStore as entry (entry.id)}
      <li>
        <ClipboardItem
          {entry}
          allTags={$tagsStore}
          {selectMode}
          selected={selectedIds.has(entry.id)}
          onPaste={() => handlePaste(entry.id)}
          onTogglePin={() => handleTogglePin(entry.id, entry.pinned)}
          onDelete={() => handleDelete(entry.id)}
          onToggleSelect={() => handleToggleSelect(entry.id)}
          onToggleTag={(tagId, checked) =>
            handleToggleTag(
              entry.id,
              entry.tags.map((t) => t.id),
              tagId,
              checked,
            )}
          onCreateTag={handleCreateTag}
          onDeleteTag={handleDeleteTag}
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
    gap: 0.4rem;
  }

  .clipboard-history__title {
    color: #f5f5f5;
    font-size: 0.85rem;
    font-weight: 600;
  }

  .clipboard-history__header-actions {
    display: flex;
    gap: 0.35rem;
  }

  .clipboard-history__clear,
  .clipboard-history__stack-toggle {
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

  .clipboard-history__search {
    background: rgba(255, 255, 255, 0.1);
    border: 1px solid rgba(255, 255, 255, 0.15);
    color: #f5f5f5;
    border-radius: 4px;
    padding: 0.3rem 0.5rem;
    font-size: 0.75rem;
  }

  .clipboard-history__tag-filters {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
  }

  .clipboard-history__tag-chip {
    background: color-mix(in srgb, var(--tag-color, rgba(255, 255, 255, 0.15)) 25%, transparent);
    border: 1px solid var(--tag-color, rgba(255, 255, 255, 0.25));
    color: #f5f5f5;
    border-radius: 999px;
    padding: 0.15rem 0.55rem;
    font-size: 0.68rem;
    cursor: pointer;
    opacity: 0.7;
  }

  .clipboard-history__tag-chip--active {
    opacity: 1;
    font-weight: 600;
  }

  .clipboard-history__stack-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: rgba(59, 130, 246, 0.18);
    border: 1px solid rgba(59, 130, 246, 0.4);
    border-radius: 4px;
    padding: 0.3rem 0.5rem;
    font-size: 0.72rem;
    color: #f5f5f5;
  }

  .clipboard-history__stack-bar button {
    background: rgba(59, 130, 246, 0.6);
    border: none;
    color: #fff;
    border-radius: 4px;
    padding: 0.2rem 0.5rem;
    font-size: 0.7rem;
    cursor: pointer;
  }

  .clipboard-history__stack-bar button:disabled {
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
