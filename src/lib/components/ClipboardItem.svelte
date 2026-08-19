<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import TagPicker from "./TagPicker.svelte";
  import type { ClipboardEntry } from "../types/clipboard";
  import type { Tag } from "../types/tags";

  interface Props {
    entry: ClipboardEntry;
    allTags: Tag[];
    /** F-15: 複数選択モード中かどうか（trueのときチェックボックスを表示する） */
    selectMode: boolean;
    /** 複数選択モード中、このアイテムが選択されているか */
    selected: boolean;
    onPaste: () => void;
    onTogglePin: () => void;
    onDelete: () => void;
    onToggleSelect: () => void;
    onToggleTag: (tagId: number, checked: boolean) => void;
    onCreateTag: (name: string, color: string) => void;
    onDeleteTag: (tagId: number) => void;
  }

  let {
    entry,
    allTags,
    selectMode,
    selected,
    onPaste,
    onTogglePin,
    onDelete,
    onToggleSelect,
    onToggleTag,
    onCreateTag,
    onDeleteTag,
  }: Props = $props();

  // サムネイルは当面フル画像を流用する（専用サムネイル生成は行わない、Phase2の簡略化）
  const thumbnailSrc = $derived(
    entry.contentType === "image" && entry.thumbnailPath ? convertFileSrc(entry.thumbnailPath) : null,
  );

  // F-15: スタック対象はtext種別のみ選択可能にする（architecture.md 9.2章）
  const selectable = $derived(entry.contentType === "text");

  let showTagPicker = $state(false);
</script>

<div class="clipboard-item" class:clipboard-item--dimmed={selectMode && !selectable}>
  <div class="clipboard-item__row">
    {#if selectMode}
      <input
        type="checkbox"
        class="clipboard-item__checkbox"
        checked={selected}
        disabled={!selectable}
        onchange={onToggleSelect}
        aria-label="結合対象として選択"
      />
    {/if}
    <button type="button" class="clipboard-item__body" onclick={onPaste} title="クリックでクリップボードへ戻す">
      {#if entry.contentType === "text"}
        <span class="clipboard-item__text">{entry.textContent ?? ""}</span>
      {:else if entry.contentType === "image" && thumbnailSrc}
        <img class="clipboard-item__thumbnail" src={thumbnailSrc} alt="コピーされた画像" />
      {:else if entry.contentType === "file_paths"}
        <span class="clipboard-item__text">{(entry.filePaths ?? []).join(", ")}</span>
      {/if}
    </button>
    <div class="clipboard-item__actions">
      <button
        type="button"
        class="clipboard-item__tag-toggle"
        class:clipboard-item__tag-toggle--active={showTagPicker}
        onclick={() => (showTagPicker = !showTagPicker)}
        aria-label="タグ付け"
      >
        🏷
      </button>
      <button
        type="button"
        class="clipboard-item__pin"
        class:clipboard-item__pin--active={entry.pinned}
        onclick={onTogglePin}
        aria-label={entry.pinned ? "ピン留めを解除" : "ピン留めする"}
      >
        📌
      </button>
      <button type="button" class="clipboard-item__remove" onclick={onDelete} aria-label="削除">×</button>
    </div>
  </div>

  {#if entry.tags.length > 0}
    <div class="clipboard-item__tags">
      {#each entry.tags as tag (tag.id)}
        <span class="clipboard-item__tag-chip" style={`--tag-color: ${tag.color ?? "#888"}`}>{tag.name}</span>
      {/each}
    </div>
  {/if}

  {#if showTagPicker}
    <TagPicker {entry} {allTags} {onToggleTag} {onCreateTag} {onDeleteTag} />
  {/if}
</div>

<style>
  .clipboard-item {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    padding: 0.4rem 0.6rem;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.08);
    color: #f5f5f5;
  }

  .clipboard-item--dimmed {
    opacity: 0.5;
  }

  .clipboard-item__row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .clipboard-item__checkbox {
    flex-shrink: 0;
    cursor: pointer;
  }

  .clipboard-item__body {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    background: none;
    border: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
    padding: 0;
    font: inherit;
  }

  .clipboard-item__text {
    width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.8rem;
  }

  .clipboard-item__thumbnail {
    max-width: 100%;
    max-height: 48px;
    border-radius: 4px;
    object-fit: contain;
  }

  .clipboard-item__actions {
    flex-shrink: 0;
    display: flex;
    gap: 0.2rem;
  }

  .clipboard-item__tag-toggle,
  .clipboard-item__pin,
  .clipboard-item__remove {
    background: none;
    border: none;
    color: inherit;
    opacity: 0.5;
    cursor: pointer;
    font-size: 0.85rem;
    line-height: 1;
    padding: 0.2rem 0.35rem;
  }

  .clipboard-item__tag-toggle--active,
  .clipboard-item__pin--active {
    opacity: 1;
  }

  .clipboard-item__tag-toggle:hover,
  .clipboard-item__pin:hover,
  .clipboard-item__remove:hover {
    opacity: 1;
  }

  .clipboard-item__tags {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
  }

  .clipboard-item__tag-chip {
    font-size: 0.65rem;
    padding: 0.1rem 0.4rem;
    border-radius: 999px;
    background: color-mix(in srgb, var(--tag-color, #888) 35%, transparent);
    border: 1px solid var(--tag-color, #888);
  }
</style>
