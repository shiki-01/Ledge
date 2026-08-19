//! 設定値からグローバルショートカットを登録/再登録する（F-01, F-20, architecture.md 5章）。
//!
//! ハードコードはせず、必ず`AppSettings`経由の値を使う。設定変更時は一度全解除してから
//! 登録し直す方針（architecture.md 5章）。

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tracing::{error, info};

use crate::error::ShelfError;
use crate::settings::AppSettings;
use crate::window;

/// `settings.shelf_hotkey`をグローバルショートカットとして登録する。
/// 既存の登録はすべて解除してから登録し直す。
pub fn register_shortcuts(app: &AppHandle, settings: &AppSettings) -> Result<(), ShelfError> {
    let manager = app.global_shortcut();

    manager
        .unregister_all()
        .map_err(|e| ShelfError::Shortcut(e.to_string()))?;

    let app_handle = app.clone();
    manager
        .on_shortcut(settings.shelf_hotkey.as_str(), move |_app, _shortcut, event| {
            // キーを押した瞬間のみ反応する（離した際のReleasedイベントでは何もしない）
            if event.state == ShortcutState::Pressed {
                window::toggle_shelf(&app_handle);
            }
        })
        .map_err(|e| {
            error!(hotkey = %settings.shelf_hotkey, error = %e, "グローバルショートカットの登録に失敗しました");
            ShelfError::Shortcut(e.to_string())
        })?;

    info!(hotkey = %settings.shelf_hotkey, "グローバルショートカットを登録しました");

    Ok(())
}
