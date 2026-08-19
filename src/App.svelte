<script lang="ts">
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import Shelf from "./lib/components/Shelf.svelte";
  import { shelfStore } from "./lib/stores/shelfStore";

  // DB更新を伴うコマンドはRust側で`shelf://items-changed`をemitするので、
  // フロントはそれを購読して自動再取得する（ポーリング不要、architecture.md 3章）
  $effect(() => {
    void shelfStore.refresh();

    let unlisten: UnlistenFn | undefined;
    void listen("shelf://items-changed", () => {
      void shelfStore.refresh();
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  });
</script>

<Shelf />
