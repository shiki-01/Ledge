//! シェルフ内アイテムを外部アプリ/フォルダへドラッグして送り出す処理（F-03, architecture.md 4.2章）。
//!
//! 受け入れ側（F-02、シェルフへのドロップ）はTauri v2組み込みのwindow drag-dropイベントで完結し、
//! ここで扱うのはアウトバウンド（シェルフ→外部）のみ。

pub mod dev_stub;

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub mod native;

use std::path::PathBuf;

use crate::error::ShelfError;

/// アウトバウンドドラッグの抽象化。
/// Windows/macOSでは`native`（tauri-plugin-drag/dragクレートのラッパー）、
/// それ以外（Linux開発環境）では`dev_stub`のno-op実装を使う。
pub trait DragOutSource: Send + Sync {
    fn begin_drag(&self, paths: Vec<PathBuf>) -> Result<(), ShelfError>;
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub fn create_drag_out_source(app_handle: tauri::AppHandle) -> Box<dyn DragOutSource> {
    Box::new(native::NativeDragOutSource::new(app_handle))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn create_drag_out_source(app_handle: tauri::AppHandle) -> Box<dyn DragOutSource> {
    Box::new(dev_stub::DevStubDragOutSource::new(app_handle))
}
