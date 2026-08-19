<script lang="ts">
  import type { ShelfItem } from "../types/shelf";

  interface Props {
    item: ShelfItem;
    onRemove: () => void;
    onDragOut: () => void;
  }

  let { item, onRemove, onDragOut }: Props = $props();

  const kindLabel = $derived(item.itemType === "folder" ? "フォルダ" : "ファイル");
</script>

<!--
  外部への再ドロップ（F-03）はHTML5のdraggable属性ではなく、mousedownを起点に
  Rust側（shelf_begin_drag_out → drag_drop::DragOutSource）へネイティブドラッグ開始を委譲する。
  Tauri v2のWindowConfigドキュメントにも「Windowsでフロントエンド側のHTML5 D&Dを使うには
  dragDropEnabledを無効化する必要がある」とあり、F-02のためdragDropEnabledをtrueにしている
  本アプリではHTML5 D&Dと共存しない設計にした（迷った設計判断）。
-->
<div class="shelf-item" class:shelf-item--missing={item.missing} title={item.sourcePath}>
  <button
    type="button"
    class="shelf-item__body"
    onmousedown={onDragOut}
    disabled={item.missing}
  >
    <span class="shelf-item__name">{item.displayName}</span>
    <span class="shelf-item__meta">
      {kindLabel}
      {#if item.missing}
        <span class="shelf-item__warning">見つかりません</span>
      {/if}
    </span>
  </button>
  <button type="button" class="shelf-item__remove" onclick={onRemove} aria-label="削除">×</button>
</div>

<style>
  .shelf-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 0.6rem;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.08);
    color: #f5f5f5;
  }

  .shelf-item--missing {
    opacity: 0.5;
  }

  .shelf-item__body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    background: none;
    border: none;
    color: inherit;
    text-align: left;
    cursor: grab;
    padding: 0;
    font: inherit;
  }

  .shelf-item__body:disabled {
    cursor: not-allowed;
  }

  .shelf-item__name {
    width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.85rem;
  }

  .shelf-item__meta {
    font-size: 0.7rem;
    opacity: 0.7;
    display: flex;
    gap: 0.4rem;
  }

  .shelf-item__warning {
    color: #ff9d9d;
  }

  .shelf-item__remove {
    flex-shrink: 0;
    background: none;
    border: none;
    color: inherit;
    opacity: 0.6;
    cursor: pointer;
    font-size: 0.9rem;
    line-height: 1;
    padding: 0.2rem 0.4rem;
  }

  .shelf-item__remove:hover {
    opacity: 1;
  }
</style>
