//! アプリ設定（ホットキー・表示位置等）の永続化。
//!
//! requirements.md 10.4章の方針どおり、設定はSQLiteではなく`tauri-plugin-store`経由の
//! `settings.json`で管理する。Phase1で実際に使うのはグローバルショートカット登録と
//! シェルフの表示端のみ（表示位置/透明度設定のUI自体はPhase3）。

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use crate::error::ShelfError;

const STORE_FILE: &str = "settings.json";
const SETTINGS_KEY: &str = "settings";

/// シェルフを表示する画面端（architecture.md 5章、既定はright）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShelfEdge {
    Top,
    Bottom,
    Left,
    Right,
}

/// アプリ設定。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// シェルフ表示/非表示を切り替えるグローバルホットキー（requirements.md D-5の暫定値）。
    pub shelf_hotkey: String,
    /// シェルフを表示する画面端。
    pub shelf_edge: ShelfEdge,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            shelf_hotkey: default_shelf_hotkey(),
            shelf_edge: ShelfEdge::Right,
        }
    }
}

// D-5: シェルフの既定ホットキーはOSごとに異なる（Ctrl+Alt+S / Cmd+Option+S）。
#[cfg(target_os = "macos")]
fn default_shelf_hotkey() -> String {
    "Cmd+Option+S".to_string()
}

#[cfg(not(target_os = "macos"))]
fn default_shelf_hotkey() -> String {
    "Ctrl+Alt+S".to_string()
}

/// `update_settings`コマンドで受け取る部分更新用の構造体（TypeScript側の`Partial<AppSettings>`に対応）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsPatch {
    pub shelf_hotkey: Option<String>,
    pub shelf_edge: Option<ShelfEdge>,
}

/// `settings.json`から設定を読み込む。未初期化、または内容が壊れている場合は
/// デフォルト値を書き込んでから返す。
pub fn load_settings(app: &AppHandle) -> Result<AppSettings, ShelfError> {
    let store = app.store(STORE_FILE).map_err(store_err)?;

    if let Some(value) = store.get(SETTINGS_KEY) {
        if let Ok(settings) = serde_json::from_value::<AppSettings>(value) {
            return Ok(settings);
        }
    }

    let defaults = AppSettings::default();
    save_settings(app, &defaults)?;
    Ok(defaults)
}

/// 設定を`settings.json`へ書き込む。
pub fn save_settings(app: &AppHandle, settings: &AppSettings) -> Result<(), ShelfError> {
    let store = app.store(STORE_FILE).map_err(store_err)?;
    let value =
        serde_json::to_value(settings).map_err(|e| ShelfError::Settings(e.to_string()))?;
    store.set(SETTINGS_KEY, value);
    store.save().map_err(store_err)?;
    Ok(())
}

/// 部分更新を適用して保存する。
pub fn update_settings(app: &AppHandle, patch: AppSettingsPatch) -> Result<AppSettings, ShelfError> {
    let mut settings = load_settings(app)?;

    if let Some(hotkey) = patch.shelf_hotkey {
        settings.shelf_hotkey = hotkey;
    }
    if let Some(edge) = patch.shelf_edge {
        settings.shelf_edge = edge;
    }

    save_settings(app, &settings)?;
    Ok(settings)
}

fn store_err(e: tauri_plugin_store::Error) -> ShelfError {
    ShelfError::Settings(e.to_string())
}
