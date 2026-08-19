//! Linux開発環境向けno-op実装。
//!
//! 実際のクリップボード監視は行わない。この開発コンテナではWindows/macOS固有コード
//! （`windows.rs` / `macos.rs`）はコンパイル対象外のため、共通部分
//! （`commands/clipboard.rs`・`storage/clipboard_repo.rs`等）の型検査・ロジック確認用に
//! この実装で代替する（architecture.md 7章、`drag_drop/dev_stub.rs`と同じパターン）。

use tracing::info;

use crate::error::ShelfError;

use super::{ClipboardSnapshot, ClipboardWatcher};

pub struct DevStubClipboardWatcher;

impl DevStubClipboardWatcher {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DevStubClipboardWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardWatcher for DevStubClipboardWatcher {
    fn start(
        &mut self,
        _on_change: Box<dyn Fn(ClipboardSnapshot) + Send + Sync>,
    ) -> Result<(), ShelfError> {
        info!("dev_stub: クリップボード監視を開始しました（no-op、Linux開発環境）");
        Ok(())
    }

    fn stop(&mut self) {
        info!("dev_stub: クリップボード監視を停止しました（no-op、Linux開発環境）");
    }
}
