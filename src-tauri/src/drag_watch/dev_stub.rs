//! macOS/Linux開発環境向けno-op実装。
//!
//! macOS側のドラッグ開始検知（F-08 Mac）はPhase5で別途検討するため、現時点ではWindows以外は
//! すべてこのno-op実装を使う（architecture.md 8.1章、`clipboard/dev_stub.rs`と同じパターン）。

use tracing::info;

use crate::error::ShelfError;

use super::DragWatcher;

pub struct DevStubDragWatcher;

impl DevStubDragWatcher {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DevStubDragWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl DragWatcher for DevStubDragWatcher {
    fn start(
        &mut self,
        _on_start: Box<dyn Fn() + Send + Sync>,
        _on_end: Box<dyn Fn() + Send + Sync>,
    ) -> Result<(), ShelfError> {
        info!("dev_stub: ドラッグ開始検知の監視を開始しました（no-op）");
        Ok(())
    }

    fn stop(&mut self) {
        info!("dev_stub: ドラッグ開始検知の監視を停止しました（no-op）");
    }
}
