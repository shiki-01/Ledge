//! 設定関連のTauriコマンド（architecture.md 3章 Phase1表、Phase3で表示位置/透明度・自動起動・
//! ドラッグ自動検知のON/OFFを追加）。

use tauri::{AppHandle, Emitter, Manager, State};

use crate::drag_watch;
use crate::error::ShelfError;
use crate::settings::{self, AppSettings, AppSettingsPatch};
use crate::shortcut;
use crate::window;
use crate::AppState;

/// 設定変更をフロントエンドへ通知するイベント名。App.svelteが購読し、`settingsStore`を
/// 再取得することで透明度（CSSの背景色）等をその場で反映する（architecture.md 8.2章）。
const EVENT_SETTINGS_CHANGED: &str = "settings://changed";

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<AppSettings, ShelfError> {
    settings::load_settings(&app)
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: AppSettingsPatch,
) -> Result<AppSettings, ShelfError> {
    let updated = settings::update_settings(&app, patch)?;

    // ホットキー変更を即座に反映する（F-20）
    shortcut::register_shortcuts(&app, &updated)?;

    // 表示位置の即時反映（F-10）。透明度はCSS側で扱うため、後述のイベント通知でフロントへ伝える。
    if let Some(win) = app.get_webview_window(window::MAIN_WINDOW_LABEL) {
        window::reposition(&win, &app);
    }

    // 自動起動設定の反映（F-19）
    apply_autostart(&app, updated.autostart_enabled)?;

    // ドラッグ開始検知の有効/無効切り替え（F-08 Windows先行）
    drag_watch::set_enabled(&app, &state.drag_watcher, updated.auto_show_on_drag_start)?;

    // フロントは購読して自動再取得するのみなので、送信失敗（購読者なし等）は無視してよい
    let _ = app.emit(EVENT_SETTINGS_CHANGED, ());

    Ok(updated)
}

/// `tauri-plugin-autostart`経由でOSログイン時自動起動のON/OFFを切り替える（F-19）。
fn apply_autostart(app: &AppHandle, enabled: bool) -> Result<(), ShelfError> {
    use tauri_plugin_autostart::ManagerExt;

    let manager = app.autolaunch();
    let result = if enabled { manager.enable() } else { manager.disable() };
    result.map_err(|e| ShelfError::Settings(format!("自動起動設定の変更に失敗しました: {e}")))
}
