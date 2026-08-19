<script lang="ts">
  /**
   * シェルフアイテムのホバー時プレビュー（F-07）。
   * 画像ファイルはTauriのasset protocol（convertFileSrc）経由で実画像を表示し、
   * それ以外は拡張子ベースの汎用アイコン＋ファイル名/サイズ/更新日時のテキスト表示に留める
   * （OSネイティブのサムネイル抽出はスコープ外、architecture.md 8.3章）。
   */
  import { convertFileSrc } from "@tauri-apps/api/core";
  import type { ShelfItem } from "../types/shelf";

  interface Props {
    item: ShelfItem;
  }

  let { item }: Props = $props();

  const IMAGE_EXTENSIONS = new Set(["png", "jpg", "jpeg", "gif", "bmp", "webp"]);

  function extensionOf(name: string): string {
    const dot = name.lastIndexOf(".");
    return dot === -1 ? "" : name.slice(dot + 1).toLowerCase();
  }

  const extension = $derived(extensionOf(item.displayName));
  const isImage = $derived(
    item.itemType === "file" && !item.missing && IMAGE_EXTENSIONS.has(extension),
  );
  const imageSrc = $derived(isImage ? convertFileSrc(item.sourcePath) : null);

  function formatSize(bytes: number | null): string {
    if (bytes === null) return "-";
    const units = ["B", "KB", "MB", "GB"];
    let value = bytes;
    let unitIndex = 0;
    while (value >= 1024 && unitIndex < units.length - 1) {
      value /= 1024;
      unitIndex += 1;
    }
    return `${value.toFixed(unitIndex === 0 ? 0 : 1)}${units[unitIndex]}`;
  }

  function formatDate(ms: number | null): string {
    if (ms === null) return "-";
    return new Date(ms).toLocaleString();
  }
</script>

<div class="shelf-item-preview">
  {#if isImage && imageSrc}
    <img class="shelf-item-preview__image" src={imageSrc} alt={item.displayName} />
  {:else}
    <div class="shelf-item-preview__icon" aria-hidden="true">
      {item.itemType === "folder" ? "📁" : "📄"}
    </div>
  {/if}
  <dl class="shelf-item-preview__meta">
    <dt>名前</dt>
    <dd>{item.displayName}</dd>
    <dt>サイズ</dt>
    <dd>{formatSize(item.sizeBytes)}</dd>
    <dt>更新日時</dt>
    <dd>{formatDate(item.modifiedAtMs)}</dd>
  </dl>
</div>

<style>
  .shelf-item-preview {
    width: 100%;
    box-sizing: border-box;
    padding: 0.5rem;
    border-radius: 6px;
    background: rgba(10, 10, 14, 0.95);
    border: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
    color: #f5f5f5;
  }

  .shelf-item-preview__image {
    display: block;
    width: 100%;
    max-height: 140px;
    object-fit: contain;
    border-radius: 4px;
    margin-bottom: 0.4rem;
    background: rgba(255, 255, 255, 0.05);
  }

  .shelf-item-preview__icon {
    font-size: 2rem;
    text-align: center;
    padding: 0.6rem 0;
  }

  .shelf-item-preview__meta {
    margin: 0;
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.15rem 0.5rem;
    font-size: 0.7rem;
  }

  .shelf-item-preview__meta dt {
    opacity: 0.6;
  }

  .shelf-item-preview__meta dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
</style>
