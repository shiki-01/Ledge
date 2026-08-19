<script lang="ts">
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import Shelf from "./lib/components/Shelf.svelte";
  import ClipboardHistory from "./lib/components/ClipboardHistory.svelte";
  import { shelfStore } from "./lib/stores/shelfStore";
  import { clipboardStore } from "./lib/stores/clipboardStore";

  type Tab = "shelf" | "history";
  let activeTab = $state<Tab>("shelf");

  // DB更新を伴うコマンドはRust側で`shelf://items-changed` / `clipboard://history-changed`をemitするので、
  // フロントはそれを購読して自動再取得する（ポーリング不要、architecture.md 3章）
  $effect(() => {
    void shelfStore.refresh();
    void clipboardStore.refresh();

    let unlistenShelf: UnlistenFn | undefined;
    let unlistenClipboard: UnlistenFn | undefined;

    void listen("shelf://items-changed", () => {
      void shelfStore.refresh();
    }).then((fn) => {
      unlistenShelf = fn;
    });

    void listen("clipboard://history-changed", () => {
      void clipboardStore.refresh();
    }).then((fn) => {
      unlistenClipboard = fn;
    });

    return () => {
      unlistenShelf?.();
      unlistenClipboard?.();
    };
  });
</script>

<div class="app">
  <nav class="app__tabs">
    <button
      type="button"
      class="app__tab"
      class:app__tab--active={activeTab === "shelf"}
      onclick={() => (activeTab = "shelf")}
    >
      シェルフ
    </button>
    <button
      type="button"
      class="app__tab"
      class:app__tab--active={activeTab === "history"}
      onclick={() => (activeTab = "history")}
    >
      履歴
    </button>
  </nav>

  <div class="app__content">
    {#if activeTab === "shelf"}
      <Shelf />
    {:else}
      <ClipboardHistory />
    {/if}
  </div>
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: rgba(20, 20, 24, 0.72);
    backdrop-filter: blur(6px);
    border: 1px solid rgba(255, 255, 255, 0.08);
  }

  .app__tabs {
    display: flex;
    flex-shrink: 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .app__tab {
    flex: 1;
    background: none;
    border: none;
    color: rgba(245, 245, 245, 0.6);
    font-size: 0.75rem;
    font-weight: 600;
    padding: 0.5rem 0;
    cursor: pointer;
  }

  .app__tab--active {
    color: #f5f5f5;
    box-shadow: inset 0 -2px 0 rgba(59, 130, 246, 0.8);
  }

  .app__content {
    flex: 1;
    min-height: 0;
  }
</style>
