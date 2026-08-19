//! F-08: OS全体でのドラッグ操作開始検知によるシェルフ自動表示（Windows先行、architecture.md 8.1章）。
//!
//! Windowsには「OS全体でドラッグ操作が始まったこと」を通知する高レベルAPIが存在しないため、
//! `WH_MOUSE_LL`（低レベルマウスフック）で「左ボタン押下→一定距離以上の移動」パターンを
//! ヒューリスティックに検知する（実装は`windows.rs`）。誤検知は許容し、シェルフの自動表示自体は
//! 非破壊的操作として扱う。macOS版（Accessibility API依存が濃厚）はPhase5で別途検討するため、
//! 本フェーズでは`dev_stub`（no-op）のみを用意する。

pub mod dev_stub;

#[cfg(target_os = "windows")]
pub mod windows;

use crate::error::ShelfError;

/// ドラッグ開始検知の抽象化。
///
/// `on_start`はヒューリスティックで「ドラッグ操作の可能性が高い」と判定された瞬間に、
/// `on_end`はドラッグ操作が終了した（ボタンを離した）とみなされた瞬間に呼ばれる。
pub trait DragWatcher: Send {
    fn start(
        &mut self,
        on_start: Box<dyn Fn() + Send + Sync>,
        on_end: Box<dyn Fn() + Send + Sync>,
    ) -> Result<(), ShelfError>;
    fn stop(&mut self);
}

/// 実行環境に応じた`DragWatcher`実装を生成する。
/// Windowsでは低レベルマウスフックによる実装、それ以外（macOS/Linux開発環境）では
/// `dev_stub`のno-op実装を使う（macOS対応はPhase5、architecture.md 8.1章）。
#[cfg(target_os = "windows")]
pub fn create_watcher() -> Box<dyn DragWatcher> {
    Box::new(windows::WindowsDragWatcher::new())
}

#[cfg(not(target_os = "windows"))]
pub fn create_watcher() -> Box<dyn DragWatcher> {
    Box::new(dev_stub::DevStubDragWatcher::new())
}

/// 設定（`AppSettings.auto_show_on_drag_start`）に応じて監視の開始/停止を切り替える。
/// `update_settings`コマンドおよび起動時セットアップの両方から呼ばれる共通処理。
///
/// 一度停止してから、有効な場合のみ開始し直す単純な方式にした（`shortcut::register_shortcuts`の
/// 「一度全解除してから登録し直す」方針に合わせた設計判断。差分検知して開始/停止を省略する最適化も
/// できるが、呼び出し頻度が低い設定変更コマンドのため単純さを優先した。呼び出し元への報告事項:
/// 迷った設計判断）。
pub fn set_enabled(
    app: &tauri::AppHandle,
    watcher: &std::sync::Mutex<Box<dyn DragWatcher>>,
    enabled: bool,
) -> Result<(), ShelfError> {
    let mut guard = watcher
        .lock()
        .map_err(|_| ShelfError::Internal("ドラッグ監視の内部ロック取得に失敗しました".into()))?;
    guard.stop();
    if enabled {
        let start_handle = app.clone();
        let end_handle = app.clone();
        guard.start(
            Box::new(move || crate::window::show_shelf_auto(&start_handle)),
            Box::new(move || crate::window::hide_shelf_if_auto(&end_handle)),
        )?;
    }
    Ok(())
}
