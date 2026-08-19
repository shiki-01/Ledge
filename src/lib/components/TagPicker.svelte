<script lang="ts">
  /**
   * 1件のクリップボードエントリに対するタグ付けUI（F-17）。
   * 既存タグへのチェックによる付け外し、新規タグ作成（名前+色）、タグ削除を行う。
   * エラー処理・実際のinvoke呼び出しは呼び出し元（ClipboardHistory.svelte）に委ね、
   * このコンポーネントはコールバック経由で通知するだけにする（他コンポーネントと同じ方針）。
   */
  import type { ClipboardEntry } from "../types/clipboard";
  import type { Tag } from "../types/tags";

  interface Props {
    entry: ClipboardEntry;
    allTags: Tag[];
    onToggleTag: (tagId: number, checked: boolean) => void;
    onCreateTag: (name: string, color: string) => void;
    onDeleteTag: (tagId: number) => void;
  }

  let { entry, allTags, onToggleTag, onCreateTag, onDeleteTag }: Props = $props();

  let newTagName = $state("");
  let newTagColor = $state("#3b82f6");

  function isAssigned(tagId: number): boolean {
    return entry.tags.some((t) => t.id === tagId);
  }

  function handleCreate(): void {
    const name = newTagName.trim();
    if (!name) return;
    onCreateTag(name, newTagColor);
    newTagName = "";
  }
</script>

<div class="tag-picker">
  {#if allTags.length > 0}
    <ul class="tag-picker__list">
      {#each allTags as tag (tag.id)}
        <li class="tag-picker__row">
          <label class="tag-picker__label">
            <input
              type="checkbox"
              checked={isAssigned(tag.id)}
              onchange={(e) => onToggleTag(tag.id, (e.target as HTMLInputElement).checked)}
            />
            <span class="tag-picker__dot" style={`--tag-color: ${tag.color ?? "#888"}`}></span>
            {tag.name}
          </label>
          <button
            type="button"
            class="tag-picker__delete"
            onclick={() => onDeleteTag(tag.id)}
            aria-label={`タグ「${tag.name}」を削除`}
          >
            ×
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="tag-picker__empty">タグがまだありません</p>
  {/if}

  <div class="tag-picker__create">
    <input
      type="text"
      class="tag-picker__name-input"
      placeholder="新しいタグ名"
      bind:value={newTagName}
      onkeydown={(e) => {
        if (e.key === "Enter") handleCreate();
      }}
    />
    <input type="color" class="tag-picker__color-input" bind:value={newTagColor} aria-label="タグの色" />
    <button type="button" class="tag-picker__add" onclick={handleCreate} disabled={!newTagName.trim()}>
      追加
    </button>
  </div>
</div>

<style>
  .tag-picker {
    background: rgba(0, 0, 0, 0.25);
    border-radius: 6px;
    padding: 0.4rem 0.5rem;
    margin-top: 0.3rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .tag-picker__list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    max-height: 120px;
    overflow-y: auto;
  }

  .tag-picker__row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 0.72rem;
  }

  .tag-picker__label {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    cursor: pointer;
  }

  .tag-picker__dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--tag-color, #888);
  }

  .tag-picker__delete {
    background: none;
    border: none;
    color: inherit;
    opacity: 0.5;
    cursor: pointer;
    font-size: 0.75rem;
    padding: 0 0.3rem;
  }

  .tag-picker__delete:hover {
    opacity: 1;
  }

  .tag-picker__empty {
    margin: 0;
    font-size: 0.7rem;
    opacity: 0.55;
  }

  .tag-picker__create {
    display: flex;
    gap: 0.3rem;
    align-items: center;
  }

  .tag-picker__name-input {
    flex: 1;
    min-width: 0;
    background: rgba(255, 255, 255, 0.1);
    border: 1px solid rgba(255, 255, 255, 0.15);
    color: inherit;
    border-radius: 4px;
    padding: 0.2rem 0.35rem;
    font-size: 0.72rem;
  }

  .tag-picker__color-input {
    width: 24px;
    height: 22px;
    padding: 0;
    border: none;
    background: none;
    cursor: pointer;
  }

  .tag-picker__add {
    background: rgba(255, 255, 255, 0.15);
    border: none;
    color: inherit;
    border-radius: 4px;
    padding: 0.2rem 0.5rem;
    font-size: 0.7rem;
    cursor: pointer;
  }

  .tag-picker__add:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
</style>
