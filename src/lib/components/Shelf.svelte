<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import ShelfItem from "./ShelfItem.svelte";
  import FavoriteFolders from "./FavoriteFolders.svelte";
  import { shelfStore } from "../stores/shelfStore";
  import {
    shelfAddPaths,
    shelfBeginDragOut,
    shelfClear,
    shelfRemoveItem,
    shelfSetLocked,
  } from "../api/commands";
  import { isShelfErrorPayload } from "../types/error";

  let isDraggingOver = $state(false);
  let errorMessage = $state<string | null>(null);
  let errorTimer: ReturnType<typeof setTimeout> | undefined;

  // F-03: シェルフからのドラッグアウト中は、送り出し中の自分自身へのOSドロップ通知
  // （送信元と受信先が同一ウィンドウという構造上、送り出し直後にOSが自己ドロップとして
  // 検知することがある）を無視するためのフラグ。`shelf://drag-out-ended`（Rust側の
  // `drag::start_drag`完了コールバック）を受け取るまでtrueのままにする。
  let isDraggingOutSelf = $state(false);
  let dragOutSafetyTimer: ReturnType<typeof setTimeout> | undefined;

  // `shelf://drag-out-ended`が何らかの理由で届かなかった場合の保険。一定時間後に自動で
  // `isDraggingOutSelf`を解除する（イベントが正常に届いた場合はclearTimeoutでキャンセルする）。
  const DRAG_OUT_SAFETY_TIMEOUT_MS = 30_000;

  function scheduleDragOutSafetyReset(): void {
    clearTimeout(dragOutSafetyTimer);
    dragOutSafetyTimer = setTimeout(() => {
      isDraggingOutSelf = false;
    }, DRAG_OUT_SAFETY_TIMEOUT_MS);
  }

  // F-03: ネイティブドラッグ操作の終了通知を購読し、自己ドロップ無視フラグを解除する
  // （既存の"shelf://open-settings"等の購読パターン、App.svelteに揃えた）。
  $effect(() => {
    let unlisten: UnlistenFn | undefined;

    void listen("shelf://drag-out-ended", () => {
      clearTimeout(dragOutSafetyTimer);
      isDraggingOutSelf = false;
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  });

  function showError(e: unknown): void {
    errorMessage = isShelfErrorPayload(e) ? e.message : "予期しないエラーが発生しました";
    clearTimeout(errorTimer);
    errorTimer = setTimeout(() => {
      errorMessage = null;
    }, 4000);
  }

  // F-02: Tauri v2組み込みのwindow drag-dropイベントを購読する
  // （tauri.conf.jsonの`dragDropEnabled: true`と対応、architecture.md 4.2章）
  $effect(() => {
    const win = getCurrentWindow();
    let unlisten: (() => void) | undefined;

    void win
      .onDragDropEvent((event) => {
        const payload = event.payload;
        switch (payload.type) {
          case "enter":
          case "over":
            // F-03自己ドロップガード: 送り出し中の自分自身へのドロップ候補通知は無視する。
            if (isDraggingOutSelf) break;
            isDraggingOver = true;
            break;
          case "drop":
            // F-03自己ドロップガード: シェルフからのドラッグアウト完了直後、OSが送り出し中の
            // ドラッグを自分自身へのドロップとして検知することがあるため無視する
            // （`isDraggingOutSelf`、`handleDragOut`参照）。
            if (isDraggingOutSelf) break;
            isDraggingOver = false;
            void handleDrop(payload.paths);
            break;
          case "leave":
            isDraggingOver = false;
            break;
        }
      })
      .then((fn) => {
        unlisten = fn;
      });

    return () => {
      unlisten?.();
    };
  });

  async function handleDrop(paths: string[]): Promise<void> {
    try {
      await shelfAddPaths(paths);
    } catch (e) {
      showError(e);
    }
  }

  async function handleRemove(id: number): Promise<void> {
    try {
      await shelfRemoveItem(id);
    } catch (e) {
      showError(e);
    }
  }

  async function handleClearAll(): Promise<void> {
    try {
      // ロック済みアイテム（F-06）は一括削除の対象から除外する
      await shelfClear(true);
    } catch (e) {
      showError(e);
    }
  }

  async function handleDragOut(id: number): Promise<void> {
    isDraggingOutSelf = true;
    scheduleDragOutSafetyReset();
    try {
      await shelfBeginDragOut([id]);
    } catch (e) {
      clearTimeout(dragOutSafetyTimer);
      isDraggingOutSelf = false; // コマンド自体が失敗した場合はOSドラッグが開始していないので即座に解除する
      showError(e);
    }
  }

  async function handleToggleLock(id: number, locked: boolean): Promise<void> {
    try {
      await shelfSetLocked(id, !locked);
    } catch (e) {
      showError(e);
    }
  }
</script>

<div class="shelf" class:shelf--dragging={isDraggingOver}>
  <header class="shelf__header">
    <span class="shelf__title">Ledge</span>
    <button
      type="button"
      class="shelf__clear"
      disabled={$shelfStore.length === 0}
      onclick={handleClearAll}
    >
      全て削除
    </button>
  </header>

  {#if errorMessage}
    <div class="shelf__toast" role="alert">{errorMessage}</div>
  {/if}

  <FavoriteFolders onError={showError} />

  <ul class="shelf__list">
    {#each $shelfStore as item (item.id)}
      <li>
        <ShelfItem
          {item}
          onRemove={() => handleRemove(item.id)}
          onDragOut={() => handleDragOut(item.id)}
          onToggleLock={() => handleToggleLock(item.id, item.locked)}
          onError={showError}
        />
      </li>
    {/each}
  </ul>

  {#if $shelfStore.length === 0}
    <p class="shelf__empty">ここにファイル/フォルダをドラッグ&ドロップしてください</p>
  {/if}
</div>

<style>
  .shelf {
    /* 背景・枠線はApp.svelteの`.app`コンテナ側で描画する（Phase2: シェルフ/履歴タブの共通chrome） */
    display: flex;
    flex-direction: column;
    height: 100%;
    box-sizing: border-box;
    padding: 0.6rem;
    gap: 0.5rem;
    transition: background-color 0.15s ease;
  }

  .shelf--dragging {
    background: rgba(59, 130, 246, 0.35);
  }

  .shelf__header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .shelf__title {
    color: #f5f5f5;
    font-size: 0.85rem;
    font-weight: 600;
  }

  .shelf__clear {
    background: rgba(255, 255, 255, 0.1);
    border: none;
    color: #f5f5f5;
    border-radius: 4px;
    padding: 0.2rem 0.5rem;
    font-size: 0.7rem;
    cursor: pointer;
  }

  .shelf__clear:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .shelf__toast {
    background: rgba(220, 38, 38, 0.85);
    color: #fff;
    font-size: 0.75rem;
    padding: 0.4rem 0.6rem;
    border-radius: 4px;
  }

  .shelf__list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    overflow-y: auto;
  }

  .shelf__empty {
    color: rgba(245, 245, 245, 0.6);
    font-size: 0.75rem;
    text-align: center;
    margin-top: 1rem;
  }
</style>
