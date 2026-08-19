<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import type { ClipboardEntry } from "../types/clipboard";

  interface Props {
    entry: ClipboardEntry;
    onPaste: () => void;
    onTogglePin: () => void;
    onDelete: () => void;
  }

  let { entry, onPaste, onTogglePin, onDelete }: Props = $props();

  // サムネイルは当面フル画像を流用する（専用サムネイル生成は行わない、Phase2の簡略化）
  const thumbnailSrc = $derived(
    entry.contentType === "image" && entry.thumbnailPath ? convertFileSrc(entry.thumbnailPath) : null,
  );
</script>

<div class="clipboard-item">
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

<style>
  .clipboard-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 0.6rem;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.08);
    color: #f5f5f5;
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

  .clipboard-item__pin--active {
    opacity: 1;
  }

  .clipboard-item__pin:hover,
  .clipboard-item__remove:hover {
    opacity: 1;
  }
</style>
