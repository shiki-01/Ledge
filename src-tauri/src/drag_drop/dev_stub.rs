//! Linux開発環境向けno-op実装。
//!
//! 実際のネイティブドラッグは行わず、ログ出力のみ行う。この開発コンテナでは
//! Windows/macOS固有コード（`native.rs`）はコンパイル対象外のため、共通部分
//! （`commands/shelf.rs`等）の型検査・ロジック確認用にこの実装で代替する
//! （architecture.md 7章）。

use std::path::PathBuf;

use tracing::info;

use crate::error::ShelfError;

use super::DragOutSource;

pub struct DevStubDragOutSource {
    // Windows/macOS実装とインターフェースを揃えるために保持するが、Phase1のno-opでは未使用
    _app_handle: tauri::AppHandle,
}

impl DevStubDragOutSource {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self {
            _app_handle: app_handle,
        }
    }
}

impl DragOutSource for DevStubDragOutSource {
    fn begin_drag(&self, paths: Vec<PathBuf>) -> Result<(), ShelfError> {
        info!(
            ?paths,
            "dev_stub: begin_drag が呼び出されました（no-op、Linux開発環境）"
        );
        Ok(())
    }
}
