//! F-08: OS全体でのドラッグ操作開始検知によるシェルフ自動表示
//! （Windows: architecture.md 8.1章、macOS: architecture.md 10.1章）。
//!
//! Windowsには「OS全体でドラッグ操作が始まったこと」を通知する高レベルAPIが存在しないため、
//! `WH_MOUSE_LL`（低レベルマウスフック）で「左ボタン押下→一定距離以上の移動」パターンを
//! ヒューリスティックに検知する（実装は`windows.rs`）。macOSでは`NSEvent`のグローバルモニタで
//! 同じヒューリスティックを踏襲する（実装は`macos.rs`、要アクセシビリティ権限）。誤検知は許容し、
//! シェルフの自動表示自体は非破壊的操作として扱う。Windows/macOS以外（Linux開発環境）では
//! `dev_stub`（no-op）を使う。

pub mod dev_stub;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

use crate::error::ShelfError;
use tauri::Manager;

/// F-08のエッジ近傍判定に使う、プライマリディスプレイ作業領域のジオメトリと設定済み表示端。
/// `window::reposition`が使うのと同じ入力（`AppSettings.shelf_edge`＋`monitor.work_area()`）から
/// 導出する。座標系は物理px・左上原点（Tauri/winitの慣例、Windows側マウス座標と1:1で一致する）。
#[derive(Debug, Clone, Copy)]
pub struct EdgeGeometry {
    pub edge: crate::settings::ShelfEdge,
    pub work_area_x: i32,
    pub work_area_y: i32,
    pub work_area_width: u32,
    pub work_area_height: u32,
    pub scale_factor: f64,
}

/// 設定済みのシェルフ表示端と、プライマリディスプレイの作業領域ジオメトリを取得する。
/// `window::reposition`と同じ情報源（`load_settings`＋`primary_monitor().work_area()`）から
/// 導出する（F-08エッジ近傍判定用）。取得に失敗した場合は`None`を返し、呼び出し元は
/// 「ゾーン制限なしで常に発火」という従来の挙動へフォールバックすること（安全側に倒し、
/// 機能そのものを丸ごと止めないため）。
pub fn compute_edge_geometry(app: &tauri::AppHandle) -> Option<EdgeGeometry> {
    let settings = crate::settings::load_settings(app).unwrap_or_default();
    let window = app.get_webview_window(crate::window::MAIN_WINDOW_LABEL)?;
    let monitor = window.primary_monitor().ok()??;
    let work_area = *monitor.work_area();
    Some(EdgeGeometry {
        edge: settings.shelf_edge,
        work_area_x: work_area.position.x,
        work_area_y: work_area.position.y,
        work_area_width: work_area.size.width,
        work_area_height: work_area.size.height,
        scale_factor: monitor.scale_factor(),
    })
}

/// ドラッグ開始検知の抽象化。
///
/// `on_start`はヒューリスティックで「ドラッグ操作の可能性が高い」と判定された瞬間に、
/// `on_end`はドラッグ操作が終了した（ボタンを離した）とみなされた瞬間に呼ばれる。
/// `edge`は設定済みシェルフ表示端の近傍にカーソルが入った時だけ発火させるためのジオメトリ
/// （誤検知が多すぎるというユーザー報告への対策、`compute_edge_geometry`参照）。`None`の場合は
/// 各実装ともゾーン制限なしで常に発火する従来の挙動にフォールバックする。
pub trait DragWatcher: Send {
    fn start(
        &mut self,
        on_start: Box<dyn Fn() + Send + Sync>,
        on_end: Box<dyn Fn() + Send + Sync>,
        edge: Option<EdgeGeometry>,
    ) -> Result<(), ShelfError>;
    fn stop(&mut self);
}

/// 実行環境に応じた`DragWatcher`実装を生成する。
/// Windowsでは低レベルマウスフック、macOSでは`NSEvent`グローバルモニタによる実装を使う
/// （architecture.md 8.1章・10.1章）。それ以外（Linux開発環境）では`dev_stub`のno-op実装を使う。
#[cfg(target_os = "windows")]
pub fn create_watcher() -> Box<dyn DragWatcher> {
    Box::new(windows::WindowsDragWatcher::new())
}

#[cfg(target_os = "macos")]
pub fn create_watcher() -> Box<dyn DragWatcher> {
    Box::new(macos::MacDragWatcher::new())
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
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
        let edge = compute_edge_geometry(app);
        guard.start(
            Box::new(move || crate::window::show_shelf_auto(&start_handle)),
            Box::new(move || crate::window::hide_shelf_if_auto(&end_handle)),
            edge,
        )?;
    }
    Ok(())
}
