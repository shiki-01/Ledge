//! Windows / macOS共通のアウトバウンドドラッグ実装。
//!
//! `tauri-plugin-drag`が内部で使用しているものと同じ`drag`クレート
//! （crates.io: <https://crates.io/crates/drag>、実装元CrabNebula）を直接呼び出す。
//! `tauri-plugin-drag`自体はJSの`invoke`経由で使うためのコマンドラッパーであり、
//! Rust側から直接ネイティブドラッグを開始したい本モジュールの用途には`drag`クレートの
//! `drag::start_drag`をそのまま使う方が素直なため、こちらを採用した
//! （呼び出し元への報告事項: 迷った設計判断）。将来フロントエンドから直接ドラッグを
//! 開始したくなった場合は、lib.rsで登録済みの`tauri_plugin_drag::init()`経由のIPCも使える。
//!
//! `#[cfg(any(target_os = "windows", target_os = "macos"))]`でのみコンパイルされるため、
//! このLinux開発コンテナでは静的なコンパイル検証ができていない（architecture.md 7章）。

use std::path::PathBuf;

use tauri::{Emitter, Manager};
use tracing::debug;

use crate::error::ShelfError;
use crate::window::MAIN_WINDOW_LABEL;

use super::DragOutSource;

pub struct NativeDragOutSource {
    app_handle: tauri::AppHandle,
}

impl NativeDragOutSource {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self { app_handle }
    }
}

impl DragOutSource for NativeDragOutSource {
    fn begin_drag(&self, paths: Vec<PathBuf>) -> Result<(), ShelfError> {
        let window = self
            .app_handle
            .get_webview_window(MAIN_WINDOW_LABEL)
            .ok_or_else(|| ShelfError::DragDropFailed("シェルフウィンドウが見つかりません".into()))?;

        // ドラッグプレビュー画像は当面アプリアイコンを流用する（専用サムネイル生成はPhase1では行わない）
        let icon_path = self
            .app_handle
            .path()
            .resolve("icons/icon.png", tauri::path::BaseDirectory::Resource)
            .map_err(|e| ShelfError::DragDropFailed(e.to_string()))?;

        let callback_paths = paths.clone();
        // ドラッグ操作が実際に終了したことをフロントエンドへ通知する（F-03「自己ドロップ」ガード用）。
        // `begin_drag`自体は`drag::start_drag`呼び出し直後に返るため、フロント側の
        // `await shelfBeginDragOut(...)`はOSドラッグが実際に終わるずっと前に解決してしまう。
        // このイベントが、フロント側がOSドラッグの終了を知る唯一の手段になる
        // （`src/lib/components/Shelf.svelte`の`isDraggingOutSelf`参照）。
        let app_handle = self.app_handle.clone();
        drag::start_drag(
            &window,
            drag::DragItem::Files(paths),
            drag::Image::File(icon_path),
            move |result, _cursor_pos| {
                debug!(?result, paths = ?callback_paths, "ネイティブドラッグ操作が終了しました");
                let _ = app_handle.emit("shelf://drag-out-ended", ());
            },
            drag::Options::default(),
        )
        .map_err(|e| ShelfError::DragDropFailed(e.to_string()))?;

        Ok(())
    }
}
