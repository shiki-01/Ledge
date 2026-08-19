<script lang="ts">
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import type { ShelfItem } from "../types/shelf";
  import ShelfItemPreview from "./ShelfItemPreview.svelte";
  import { shelfCompressItem, shelfCopyPath } from "../api/commands";

  interface Props {
    item: ShelfItem;
    onRemove: () => void;
    onDragOut: () => void;
    onToggleLock: () => void;
    /** エラー表示はShelf.svelte側のトーストへ集約する（onRemove等と同じprops経由コールバックのパターン） */
    onError: (e: unknown) => void;
  }

  let { item, onRemove, onDragOut, onToggleLock, onError }: Props = $props();

  const kindLabel = $derived(item.itemType === "folder" ? "フォルダ" : "ファイル");

  // F-21: 右クリックメニュー（ネイティブメニューAPIではなくCSS/HTML製ポップアップ + JS状態管理。
  // 理由はarchitecture.md 12.2章参照: 常時最前面・装飾無しの300px幅ウィンドウというこのアプリ特有の
  // ウィンドウ形状に対して、ネイティブメニューの表示位置調整より確実に動作するための判断）。
  let menuOpen = $state(false);
  let menuPosition = $state({ x: 0, y: 0 });
  let menuElement: HTMLDivElement | undefined = $state();

  function openMenu(e: MouseEvent): void {
    e.preventDefault();
    menuPosition = { x: e.clientX, y: e.clientY };
    menuOpen = true;
  }

  function closeMenu(): void {
    menuOpen = false;
  }

  // メニュー表示中のみwindowのclickを監視し、メニュー外クリックで閉じる
  $effect(() => {
    if (!menuOpen) {
      return;
    }
    function handleWindowClick(e: MouseEvent): void {
      if (menuElement && !menuElement.contains(e.target as Node)) {
        closeMenu();
      }
    }
    window.addEventListener("click", handleWindowClick);
    return () => {
      window.removeEventListener("click", handleWindowClick);
    };
  });

  // ウィンドウ幅が既定300pxと狭いため、右クリック位置基準のままだとメニューがウィンドウ枠の
  // 外にはみ出してクリックできなくなる場合がある。描画後の実サイズを見てウィンドウ内に収める
  // （迷った設計判断: あらかじめメニュー幅を定数で決め打ちする代替案もあったが、項目数が
  // 増えた際にズレるため実測クランプを選んだ）。
  $effect(() => {
    if (!menuOpen || !menuElement) {
      return;
    }
    const rect = menuElement.getBoundingClientRect();
    const maxX = Math.max(window.innerWidth - rect.width - 4, 0);
    const maxY = Math.max(window.innerHeight - rect.height - 4, 0);
    if (menuPosition.x > maxX || menuPosition.y > maxY) {
      menuPosition = { x: Math.min(menuPosition.x, maxX), y: Math.min(menuPosition.y, maxY) };
    }
  });

  async function handleCopyPath(): Promise<void> {
    closeMenu();
    try {
      await shelfCopyPath(item.id);
    } catch (e) {
      onError(e);
    }
  }

  async function handleCompress(): Promise<void> {
    closeMenu();
    try {
      // 成功するとRust側がshelf://items-changedをemitするので、ストアは自動更新される
      // （フロント側で個別に状態更新する必要はない）
      await shelfCompressItem(item.id);
    } catch (e) {
      onError(e);
    }
  }

  async function handleRevealInDir(): Promise<void> {
    closeMenu();
    try {
      await revealItemInDir(item.sourcePath);
    } catch (e) {
      onError(e);
    }
  }
</script>

<!--
  外部への再ドロップ（F-03）はHTML5のdraggable属性ではなく、mousedownを起点に
  Rust側（shelf_begin_drag_out → drag_drop::DragOutSource）へネイティブドラッグ開始を委譲する。
  Tauri v2のWindowConfigドキュメントにも「Windowsでフロントエンド側のHTML5 D&Dを使うには
  dragDropEnabledを無効化する必要がある」とあり、F-02のためdragDropEnabledをtrueにしている
  本アプリではHTML5 D&Dと共存しない設計にした（迷った設計判断）。

  プレビュー（F-07）はホバー時にCSSのみで表示するポップオーバーとする。JS側でホバー状態を
  管理しない分シンプルだが、位置計算はCSSのabsolute配置に限定される（迷った設計判断）。
-->
<div class="shelf-item-wrapper">
  <div
    class="shelf-item"
    class:shelf-item--missing={item.missing}
    title={item.sourcePath}
    oncontextmenu={openMenu}
    role="group"
    aria-label={`${item.displayName}の操作`}
  >
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
    <button
      type="button"
      class="shelf-item__lock"
      class:shelf-item__lock--active={item.locked}
      onclick={onToggleLock}
      aria-label={item.locked ? "ロックを解除" : "ロックする"}
      title={item.locked ? "ロック中（全て削除の対象外）" : "ロックする"}
    >
      {item.locked ? "🔒" : "🔓"}
    </button>
    <button type="button" class="shelf-item__remove" onclick={onRemove} aria-label="削除">×</button>
  </div>

  <div class="shelf-item__preview-popover">
    <ShelfItemPreview {item} />
  </div>
</div>

{#if menuOpen}
  <div
    bind:this={menuElement}
    class="shelf-item__context-menu"
    style={`left: ${menuPosition.x}px; top: ${menuPosition.y}px;`}
    role="menu"
  >
    <button type="button" class="shelf-item__context-menu-item" onclick={handleCopyPath}>
      パスをコピー
    </button>
    <button
      type="button"
      class="shelf-item__context-menu-item"
      onclick={handleCompress}
      disabled={item.missing}
    >
      圧縮してシェルフに追加
    </button>
    <button
      type="button"
      class="shelf-item__context-menu-item"
      onclick={handleRevealInDir}
      disabled={item.missing}
    >
      エクスプローラー/Finderで表示
    </button>
  </div>
{/if}

<style>
  .shelf-item-wrapper {
    position: relative;
  }

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

  .shelf-item__lock,
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

  .shelf-item__lock--active {
    opacity: 1;
  }

  .shelf-item__lock:hover,
  .shelf-item__remove:hover {
    opacity: 1;
  }

  /*
    F-07: ホバーで表示するプレビューポップオーバー（CSSのみで開閉、JS側で状態管理しない）。
    シェルフウィンドウ自体が画面端に固定された狭い縦長ウィンドウ（既定幅300px）のため、
    項目の左右にポップオーバーを出すとウィンドウ枠の外にはみ出してクリップされてしまう。
    そのため項目の直下に、ウィンドウ幅に収まる形で展開する設計にした（迷った設計判断）。
  */
  .shelf-item__preview-popover {
    display: none;
    position: absolute;
    left: 0;
    right: 0;
    top: 100%;
    margin-top: 0.25rem;
    z-index: 10;
  }

  .shelf-item-wrapper:hover .shelf-item__preview-popover {
    display: block;
  }

  /*
    F-21: 右クリックメニュー。ウィンドウが常時最前面・装飾無しの狭いウィンドウのため、
    クリック座標基準のposition: fixedで表示する（ネイティブメニューAPIを使わない理由は
    ShelfItem.svelte冒頭のコメント参照）。
  */
  .shelf-item__context-menu {
    position: fixed;
    z-index: 100;
    min-width: 180px;
    background: rgba(30, 30, 34, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 6px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
    padding: 0.25rem;
    display: flex;
    flex-direction: column;
  }

  .shelf-item__context-menu-item {
    background: none;
    border: none;
    color: #f5f5f5;
    text-align: left;
    font-size: 0.8rem;
    padding: 0.4rem 0.6rem;
    border-radius: 4px;
    cursor: pointer;
  }

  .shelf-item__context-menu-item:hover {
    background: rgba(255, 255, 255, 0.1);
  }

  .shelf-item__context-menu-item:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .shelf-item__context-menu-item:disabled:hover {
    background: none;
  }
</style>
