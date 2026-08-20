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
/// F-22（デバイス間同期）用Firebaseサインインパスワードの保存キー。`AppSettings`本体とは
/// 別キーにして`settings.json`内でも分離し、`get_settings`（`AppSettings`をまるごとシリアライズして
/// フロントへ返す）のレスポンス経路に平文パスワードが絶対に乗らないようにする
/// （迷った設計判断: architecture.md 10.2章はOSキーチェーン連携を将来課題として明示的にスコープ外に
/// しているため、`tauri-plugin-store`の別キーへ保存する方式を採った。設定ファイル自体の秘匿度は
/// 既存の`settings.json`と同水準）。
const SYNC_PASSWORD_KEY: &str = "sync_firebase_password";

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
    /// クリップボード履歴の自動クリア（F-16）: 件数上限。既定500件（requirements.md 10.2章）。
    /// 設定画面UI自体はPhase3のため、Phase2では初期値のみ持たせる。
    /// `#[serde(default)]`により、Phase1時点の`settings.json`（このフィールドを持たない）を
    /// 読み込んでも既存のホットキー等を失わずに済むようにしている。
    #[serde(default = "default_clipboard_max_entries")]
    pub clipboard_max_entries: u32,
    /// クリップボード履歴の自動クリア（F-16）: 経過日数上限。既定30日（requirements.md 10.2章）。
    #[serde(default = "default_clipboard_retention_days")]
    pub clipboard_retention_days: u32,
    /// シェルフ背景の透明度（F-10, Phase3）。0.0（完全透明）〜1.0（不透明）、既定0.85。
    /// ウィンドウ全体ではなく背景要素へのCSS適用で反映する（architecture.md 8.2章）。
    #[serde(default = "default_opacity")]
    pub opacity: f64,
    /// OS起動時の自動起動ON/OFF（F-19, Phase3）。既定false（明示的に有効化するまでは起動しない）。
    #[serde(default)]
    pub autostart_enabled: bool,
    /// ドラッグ開始検知によるシェルフ自動表示のON/OFF（F-08 Windows先行, Phase3）。
    /// 既定true（architecture.md 8.1章）。誤検知が気になる場合はOFFにできる。
    #[serde(default = "default_auto_show_on_drag_start")]
    pub auto_show_on_drag_start: bool,
    /// デバイス間同期（F-22, Bring Your Own Firebase方式）の有効/無効。既定false
    /// （完全にオプトインとし、未設定時は従来どおりローカルSQLiteのみで完結する。
    /// architecture.md 10.2章）。
    #[serde(default)]
    pub sync_enabled: bool,
    /// Firebase Web SDK構成: apiKey。ユーザー自身のFirebaseプロジェクトのものを入力する。
    #[serde(default)]
    pub firebase_api_key: Option<String>,
    /// Firebase Web SDK構成: authDomain。
    #[serde(default)]
    pub firebase_auth_domain: Option<String>,
    /// Firebase Web SDK構成: projectId。
    #[serde(default)]
    pub firebase_project_id: Option<String>,
    /// Firebase Web SDK構成: appId。
    #[serde(default)]
    pub firebase_app_id: Option<String>,
    /// Firebase Authentication（Email/Password）のサインイン用Email。
    /// パスワードはここに含めない（`SYNC_PASSWORD_KEY`側、上記コメント参照）。
    #[serde(default)]
    pub firebase_email: Option<String>,
}

fn default_clipboard_max_entries() -> u32 {
    500
}

fn default_clipboard_retention_days() -> u32 {
    30
}

fn default_opacity() -> f64 {
    0.85
}

fn default_auto_show_on_drag_start() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            shelf_hotkey: default_shelf_hotkey(),
            shelf_edge: ShelfEdge::Right,
            clipboard_max_entries: default_clipboard_max_entries(),
            clipboard_retention_days: default_clipboard_retention_days(),
            opacity: default_opacity(),
            autostart_enabled: false,
            auto_show_on_drag_start: default_auto_show_on_drag_start(),
            sync_enabled: false,
            firebase_api_key: None,
            firebase_auth_domain: None,
            firebase_project_id: None,
            firebase_app_id: None,
            firebase_email: None,
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
    pub clipboard_max_entries: Option<u32>,
    pub clipboard_retention_days: Option<u32>,
    pub opacity: Option<f64>,
    pub autostart_enabled: Option<bool>,
    pub auto_show_on_drag_start: Option<bool>,
    pub sync_enabled: Option<bool>,
    pub firebase_api_key: Option<String>,
    pub firebase_auth_domain: Option<String>,
    pub firebase_project_id: Option<String>,
    pub firebase_app_id: Option<String>,
    pub firebase_email: Option<String>,
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
    let value = serde_json::to_value(settings).map_err(|e| ShelfError::Settings(e.to_string()))?;
    store.set(SETTINGS_KEY, value);
    store.save().map_err(store_err)?;
    Ok(())
}

/// 部分更新を適用して保存する。
pub fn update_settings(
    app: &AppHandle,
    patch: AppSettingsPatch,
) -> Result<AppSettings, ShelfError> {
    let mut settings = load_settings(app)?;

    if let Some(hotkey) = patch.shelf_hotkey {
        settings.shelf_hotkey = hotkey;
    }
    if let Some(edge) = patch.shelf_edge {
        settings.shelf_edge = edge;
    }
    if let Some(max_entries) = patch.clipboard_max_entries {
        settings.clipboard_max_entries = max_entries;
    }
    if let Some(retention_days) = patch.clipboard_retention_days {
        settings.clipboard_retention_days = retention_days;
    }
    if let Some(opacity) = patch.opacity {
        // 不正な範囲値が渡された場合に備えて0.0〜1.0にclampする（フロント側のrangeスライダーは
        // min/maxで制限しているが、コマンドは直接呼び出されうるため防御的に扱う）
        settings.opacity = clamp_opacity(opacity);
    }
    if let Some(autostart_enabled) = patch.autostart_enabled {
        settings.autostart_enabled = autostart_enabled;
    }
    if let Some(auto_show_on_drag_start) = patch.auto_show_on_drag_start {
        settings.auto_show_on_drag_start = auto_show_on_drag_start;
    }
    if let Some(sync_enabled) = patch.sync_enabled {
        settings.sync_enabled = sync_enabled;
    }
    // Firebase構成の各フィールドはpatchで渡された場合のみ上書きする（既存の他フィールドと同じ
    // 部分更新パターン）。空文字での明示的なクリアはフロント側から`Some("".to_string())`を
    // 渡すことで表現する想定（他の文字列フィールドと同様、専用のクリア用sentinelは設けない）。
    if let Some(api_key) = patch.firebase_api_key {
        settings.firebase_api_key = Some(api_key);
    }
    if let Some(auth_domain) = patch.firebase_auth_domain {
        settings.firebase_auth_domain = Some(auth_domain);
    }
    if let Some(project_id) = patch.firebase_project_id {
        settings.firebase_project_id = Some(project_id);
    }
    if let Some(app_id) = patch.firebase_app_id {
        settings.firebase_app_id = Some(app_id);
    }
    if let Some(email) = patch.firebase_email {
        settings.firebase_email = Some(email);
    }

    save_settings(app, &settings)?;
    Ok(settings)
}

/// F-22同期用Firebaseサインインパスワードを`settings.json`の別キー（`SYNC_PASSWORD_KEY`）へ
/// 書き込む。`AppSettings`本体には含めないため、`update_settings`とは別のTauriコマンドから
/// 呼び出す（`commands::settings::sync_set_firebase_password`）。
/// `None`または空文字を渡すとパスワードを削除する（サインアウト・入力欄クリア時の想定）。
pub fn set_firebase_password(app: &AppHandle, password: Option<&str>) -> Result<(), ShelfError> {
    let store = app.store(STORE_FILE).map_err(store_err)?;
    match password {
        Some(pw) if !pw.is_empty() => {
            store.set(SYNC_PASSWORD_KEY, serde_json::Value::String(pw.to_string()));
        }
        _ => {
            store.delete(SYNC_PASSWORD_KEY);
        }
    }
    store.save().map_err(store_err)?;
    Ok(())
}

fn store_err(e: tauri_plugin_store::Error) -> ShelfError {
    ShelfError::Settings(e.to_string())
}

/// 透明度を0.0〜1.0へclampする（`update_settings`から呼ばれる、単体テスト用に切り出し）。
fn clamp_opacity(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_have_expected_values() {
        let settings = AppSettings::default();
        assert_eq!(settings.shelf_edge, ShelfEdge::Right);
        assert_eq!(settings.clipboard_max_entries, 500);
        assert_eq!(settings.clipboard_retention_days, 30);
        assert!((settings.opacity - 0.85).abs() < f64::EPSILON);
        assert!(!settings.autostart_enabled);
        assert!(settings.auto_show_on_drag_start);
        assert!(!settings.sync_enabled);
        assert_eq!(settings.firebase_api_key, None);
        assert_eq!(settings.firebase_auth_domain, None);
        assert_eq!(settings.firebase_project_id, None);
        assert_eq!(settings.firebase_app_id, None);
        assert_eq!(settings.firebase_email, None);
    }

    #[test]
    fn serde_round_trip_preserves_all_fields() {
        let settings = AppSettings {
            shelf_hotkey: "Ctrl+Alt+S".to_string(),
            shelf_edge: ShelfEdge::Top,
            clipboard_max_entries: 100,
            clipboard_retention_days: 7,
            opacity: 0.5,
            autostart_enabled: true,
            auto_show_on_drag_start: false,
            sync_enabled: true,
            firebase_api_key: Some("api-key".to_string()),
            firebase_auth_domain: Some("example.firebaseapp.com".to_string()),
            firebase_project_id: Some("example-project".to_string()),
            firebase_app_id: Some("1:123:web:abc".to_string()),
            firebase_email: Some("sync@example.com".to_string()),
        };
        let json = serde_json::to_string(&settings).unwrap();
        let restored: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.shelf_hotkey, settings.shelf_hotkey);
        assert_eq!(restored.shelf_edge, settings.shelf_edge);
        assert_eq!(
            restored.clipboard_max_entries,
            settings.clipboard_max_entries
        );
        assert_eq!(
            restored.clipboard_retention_days,
            settings.clipboard_retention_days
        );
        assert!((restored.opacity - settings.opacity).abs() < f64::EPSILON);
        assert_eq!(restored.autostart_enabled, settings.autostart_enabled);
        assert_eq!(
            restored.auto_show_on_drag_start,
            settings.auto_show_on_drag_start
        );
        assert_eq!(restored.sync_enabled, settings.sync_enabled);
        assert_eq!(restored.firebase_api_key, settings.firebase_api_key);
        assert_eq!(restored.firebase_auth_domain, settings.firebase_auth_domain);
        assert_eq!(restored.firebase_project_id, settings.firebase_project_id);
        assert_eq!(restored.firebase_app_id, settings.firebase_app_id);
        assert_eq!(restored.firebase_email, settings.firebase_email);
    }

    /// Phase1/2時点の`settings.json`（本フェーズで追加したフィールドを持たない）を読み込んでも
    /// 既定値で補完され、既存フィールドは維持されることを確認する（`#[serde(default = ...)]`の検証）。
    #[test]
    fn deserializing_legacy_settings_json_fills_new_fields_with_defaults() {
        let legacy_json = r#"{
            "shelfHotkey": "Ctrl+Alt+S",
            "shelfEdge": "left",
            "clipboardMaxEntries": 200,
            "clipboardRetentionDays": 14
        }"#;
        let settings: AppSettings = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(settings.shelf_hotkey, "Ctrl+Alt+S");
        assert_eq!(settings.shelf_edge, ShelfEdge::Left);
        assert_eq!(settings.clipboard_max_entries, 200);
        assert_eq!(settings.clipboard_retention_days, 14);
        assert!((settings.opacity - 0.85).abs() < f64::EPSILON);
        assert!(!settings.autostart_enabled);
        assert!(settings.auto_show_on_drag_start);
        assert!(!settings.sync_enabled);
        assert_eq!(settings.firebase_api_key, None);
        assert_eq!(settings.firebase_auth_domain, None);
        assert_eq!(settings.firebase_project_id, None);
        assert_eq!(settings.firebase_app_id, None);
        assert_eq!(settings.firebase_email, None);
    }

    #[test]
    fn clamp_opacity_restricts_to_unit_range() {
        assert!((clamp_opacity(-0.5) - 0.0).abs() < f64::EPSILON);
        assert!((clamp_opacity(1.5) - 1.0).abs() < f64::EPSILON);
        assert!((clamp_opacity(0.42) - 0.42).abs() < f64::EPSILON);
    }
}
