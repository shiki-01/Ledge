<script lang="ts">
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import Shelf from "./lib/components/Shelf.svelte";
  import ClipboardHistory from "./lib/components/ClipboardHistory.svelte";
  import Settings from "./lib/components/Settings.svelte";
  import { shelfStore } from "./lib/stores/shelfStore";
  import { clipboardStore } from "./lib/stores/clipboardStore";
  import { settingsStore } from "./lib/stores/settingsStore";
  import { tagsStore } from "./lib/stores/tagsStore";
  import { favoritesStore } from "./lib/stores/favoritesStore";
  import {
    startClipboardSync,
    stopClipboardSync,
    pushPinnedTextEntries,
    isApplyingRemoteChange,
  } from "./lib/sync/clipboardSync";
  import type { FirebaseConfig } from "./lib/types/settings";

  type Tab = "shelf" | "history" | "settings";
  let activeTab = $state<Tab>("shelf");

  // DB更新を伴うコマンドはRust側で`shelf://items-changed` / `clipboard://history-changed`をemitするので、
  // フロントはそれを購読して自動再取得する（ポーリング不要、architecture.md 3章）。
  // 設定変更（`settings://changed`）とトレイの「設定...」（`shelf://open-settings`）も同様に購読する。
  // タグ一覧の増減（`tags://changed`）はPhase4 F-17で追加（architecture.md 9.3章）。
  // よく使うフォルダの増減（`favorites://changed`）はPhase6 F-09で追加（architecture.md 12.1章）。
  $effect(() => {
    void shelfStore.refresh();
    void clipboardStore.refresh();
    void settingsStore.refresh();
    void tagsStore.refresh();
    void favoritesStore.refresh();

    let unlistenShelf: UnlistenFn | undefined;
    let unlistenClipboard: UnlistenFn | undefined;
    let unlistenSettings: UnlistenFn | undefined;
    let unlistenOpenSettings: UnlistenFn | undefined;
    let unlistenTags: UnlistenFn | undefined;
    let unlistenFavorites: UnlistenFn | undefined;

    void listen("shelf://items-changed", () => {
      void shelfStore.refresh();
    }).then((fn) => {
      unlistenShelf = fn;
    });

    void listen("clipboard://history-changed", () => {
      void clipboardStore.refresh();
      // F-22（デバイス間同期）: pull（クラウド→ローカル反映）由来の変更をそのままpushし返すと
      // 無限ループになるため、pull処理中はpushをスキップする（isApplyingRemoteChange、
      // 迷った設計判断はclipboardSync.ts参照）。
      if (!isApplyingRemoteChange()) {
        void pushPinnedTextEntries();
      }
    }).then((fn) => {
      unlistenClipboard = fn;
    });

    void listen("settings://changed", () => {
      void settingsStore.refresh();
    }).then((fn) => {
      unlistenSettings = fn;
    });

    // トレイメニューの「設定...」から開かれた場合、設定タブへ切り替える
    void listen("shelf://open-settings", () => {
      activeTab = "settings";
    }).then((fn) => {
      unlistenOpenSettings = fn;
    });

    void listen("tags://changed", () => {
      void tagsStore.refresh();
    }).then((fn) => {
      unlistenTags = fn;
    });

    void listen("favorites://changed", () => {
      void favoritesStore.refresh();
    }).then((fn) => {
      unlistenFavorites = fn;
    });

    return () => {
      unlistenShelf?.();
      unlistenClipboard?.();
      unlistenSettings?.();
      unlistenOpenSettings?.();
      unlistenTags?.();
      unlistenFavorites?.();
    };
  });

  // F-10: 透明度はウィンドウ全体ではなく背景要素へのCSS適用で反映する（architecture.md 8.2章）
  const backgroundOpacity = $derived($settingsStore?.opacity ?? 0.85);

  // F-22（デバイス間同期）: syncEnabled かつ Firebase構成一式が揃っている場合のみ同期エンジンを
  // 起動する。設定オブジェクト全体を`$effect`内で読むと無関係な設定変更（透明度など）のたびに
  // リスナーを張り直してしまうため、関連フィールドだけを`$derived`でまとめた署名文字列を作り、
  // それが変化したときだけ`$effect`を再実行する（迷った設計判断: 呼び出し元へ報告）。
  const syncConfigSignature = $derived.by(() => {
    const s = $settingsStore;
    if (!s?.syncEnabled || !s.firebaseApiKey || !s.firebaseAuthDomain || !s.firebaseProjectId || !s.firebaseAppId) {
      return null;
    }
    const config: FirebaseConfig = {
      apiKey: s.firebaseApiKey,
      authDomain: s.firebaseAuthDomain,
      projectId: s.firebaseProjectId,
      appId: s.firebaseAppId,
    };
    return JSON.stringify(config);
  });

  $effect(() => {
    const signature = syncConfigSignature;
    if (signature) {
      void startClipboardSync(JSON.parse(signature) as FirebaseConfig);
    } else {
      stopClipboardSync();
    }
    return () => {
      stopClipboardSync();
    };
  });
</script>

<div class="app" style={`--shelf-bg-opacity: ${backgroundOpacity}`}>
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
    <button
      type="button"
      class="app__tab"
      class:app__tab--active={activeTab === "settings"}
      onclick={() => (activeTab = "settings")}
    >
      設定
    </button>
  </nav>

  <div class="app__content">
    {#if activeTab === "shelf"}
      <Shelf />
    {:else if activeTab === "history"}
      <ClipboardHistory />
    {:else}
      <Settings />
    {/if}
  </div>
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100%;
    /* F-10: 透明度は--shelf-bg-opacity（settingsStore.opacityから設定）で決まる。既定0.85未取得時は0.72 */
    background: rgba(20, 20, 24, var(--shelf-bg-opacity, 0.72));
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
