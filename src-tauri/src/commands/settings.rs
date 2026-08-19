//! 設定関連のTauriコマンド（architecture.md 3章 Phase1表）。

use tauri::AppHandle;

use crate::error::ShelfError;
use crate::settings::{self, AppSettings, AppSettingsPatch};
use crate::shortcut;

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<AppSettings, ShelfError> {
    settings::load_settings(&app)
}

#[tauri::command]
pub fn update_settings(app: AppHandle, patch: AppSettingsPatch) -> Result<AppSettings, ShelfError> {
    let updated = settings::update_settings(&app, patch)?;
    // ホットキー変更を即座に反映する（表示位置は次回シェルフ表示時に再計算される）
    shortcut::register_shortcuts(&app, &updated)?;
    Ok(updated)
}
