//! クリップボード監視（F-11, architecture.md 4.1章）。
//!
//! OS依存の監視処理は`windows.rs` / `macos.rs` / `dev_stub.rs`へ分岐し、
//! `commands/clipboard.rs`からは本モジュールの`ClipboardWatcher` trait越しにのみ触る。

pub mod dev_stub;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

use std::path::PathBuf;
use std::sync::Mutex;

use crate::error::ShelfError;

/// 監視で検知したクリップボードの内容（architecture.md 4.1章）。
pub enum ClipboardSnapshot {
    Text(String),
    /// PNGエンコード済みバイト列
    Image(Vec<u8>),
    FilePaths(Vec<PathBuf>),
}

/// クリップボード監視の抽象化。
///
/// 除外規約（requirements.md 10.3章: `ExcludeClipboardContentFromMonitorProcessing` /
/// `org.nspasteboard.*`）のチェックは実装側（各OS impl）で行い、除外対象は
/// コールバックを呼ばずに握りつぶす。
pub trait ClipboardWatcher: Send {
    /// 監視を開始し、変更検知のたびにコールバックを呼ぶ。
    fn start(
        &mut self,
        on_change: Box<dyn Fn(ClipboardSnapshot) + Send + Sync>,
    ) -> Result<(), ShelfError>;
    fn stop(&mut self);
}

/// 実行環境に応じた`ClipboardWatcher`実装を生成する。
/// Windows/macOSでは各OS実装、それ以外（Linux開発環境）では`dev_stub`のno-op実装を使う
/// （architecture.md 7章）。
#[cfg(target_os = "windows")]
pub fn create_watcher() -> Box<dyn ClipboardWatcher> {
    Box::new(windows::WindowsClipboardWatcher::new())
}

#[cfg(target_os = "macos")]
pub fn create_watcher() -> Box<dyn ClipboardWatcher> {
    Box::new(macos::MacClipboardWatcher::new())
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn create_watcher() -> Box<dyn ClipboardWatcher> {
    Box::new(dev_stub::DevStubClipboardWatcher::new())
}

/// 自己ループ防止用のガード（requirements.md 10.2章）。
///
/// F-12（履歴からの貼り付け）でアプリ自身がクリップボードへ書き込む直前に、
/// 書き込む内容のcontent hashを`mark`しておく。監視コールバック側は変更検知のたびに
/// `consume_if_matches`を呼び、一致すれば「自己書き込みによる変更」と判断して
/// 履歴への再記録をスキップする（一致した時点でガードは消費され、以降の変更検知には影響しない）。
pub struct SelfWriteGuard(Mutex<Option<String>>);

impl SelfWriteGuard {
    pub fn new() -> Self {
        Self(Mutex::new(None))
    }

    pub fn mark(&self, hash: String) {
        *self.lock() = Some(hash);
    }

    /// 直前に`mark`された値と一致すれば消費してtrueを返す。一致しなければfalse（何もしない）。
    pub fn consume_if_matches(&self, hash: &str) -> bool {
        let mut guard = self.lock();
        if guard.as_deref() == Some(hash) {
            *guard = None;
            true
        } else {
            false
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<String>> {
        // 万一どこかのスレッドがpanicしてロックが汚染されても、監視自体は継続させたいため
        // 汚染を回収して続行する（PoisonErrorをそのままpanicさせない）。
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Default for SelfWriteGuard {
    fn default() -> Self {
        Self::new()
    }
}
