import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Tauriが期待する固定ポートで待ち受ける（tauri.conf.jsonのdevUrlと対応）
const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [svelte()],

  // Tauri v2の推奨設定: dev serverの挙動をTauriに合わせる
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // src-tauri配下の変更はTauri側のホットリロードに任せる
      ignored: ["**/src-tauri/**"],
    },
  },
}));
