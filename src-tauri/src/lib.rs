//! Tauri Builderの組み立て、プラグイン登録、`run()`のエントリポイント。
//! 実際の起動処理は`main.rs`から`run()`を呼ぶだけの薄い委譲構成にする（architecture.md 1章）。

pub mod commands;
pub mod drag_drop;
pub mod error;
pub mod settings;
pub mod shortcut;
pub mod storage;
pub mod tray;
pub mod window;

// クリップボード監視（F-11以降）はPhase2で実装する。ディレクトリのみ先行して用意しており、
// Phase1では未使用のためmod宣言はしていない（src/clipboard/mod.rs参照）。

use std::path::PathBuf;

use tauri::Manager;
use tracing_subscriber::EnvFilter;

use error::ShelfError;
use storage::db::Db;

/// アプリ全体で共有する状態。`tauri::State`経由で各コマンドから参照する。
pub struct AppState {
    pub db: Db,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        // 多重起動防止（architecture.md 5章）。2重起動時は既存インスタンスのシェルフを表示する。
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            window::show_shelf(app);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::new().build());

    // F-03（アウトバウンドドラッグ）用プラグイン。Rust側からは`drag`クレートを直接呼んでいるため
    // 必須ではないが、将来フロントエンドから直接ドラッグを開始したくなった場合に備えて登録しておく
    // （タスク指示: 依存クレートとしてtauri-plugin-dragを追加すること）。
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    let builder = builder.plugin(tauri_plugin_drag::init());

    builder
        .setup(|app| {
            let app_handle = app.handle().clone();

            init_logging(&app_handle)?;

            let db_path = resolve_db_path(&app_handle)?;
            let db = Db::connect(&db_path)?;
            app.manage(AppState { db });

            tray::setup_tray(&app_handle)?;

            let settings = settings::load_settings(&app_handle)?;
            shortcut::register_shortcuts(&app_handle, &settings)?;

            // 起動時にシェルフを既定の画面端（右端）へ配置しておく（F-01, architecture.md 5章）。
            // 表示自体はホットキー/トレイ操作をトリガーとし、起動直後は非表示のままにする
            // （tauri.conf.jsonの`visible: false`と対応）。
            if let Some(window) = app.get_webview_window(window::MAIN_WINDOW_LABEL) {
                window::reposition(&window, &app_handle);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::shelf::shelf_list_items,
            commands::shelf::shelf_add_paths,
            commands::shelf::shelf_remove_item,
            commands::shelf::shelf_clear,
            commands::shelf::shelf_begin_drag_out,
            commands::settings::get_settings,
            commands::settings::update_settings,
        ])
        .run(tauri::generate_context!())
        .expect("shelf-drop の起動に失敗しました");
}

/// SQLiteファイルの保存先（アプリデータディレクトリ配下）を解決する。
fn resolve_db_path(app: &tauri::AppHandle) -> Result<PathBuf, ShelfError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| ShelfError::Internal(format!("アプリデータディレクトリの解決に失敗しました: {e}")))?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| ShelfError::Internal(format!("アプリデータディレクトリの作成に失敗しました: {e}")))?;
    Ok(dir.join("shelf-drop.sqlite3"))
}

/// `tracing` + `tracing-appender`によるログのローテーション出力を初期化する（requirements.md 11章）。
/// クリップボードの内容そのものはログに出力しない方針のため、Phase1時点でも
/// イベント種別・エラー内容程度に留める運用を前提とする。
fn init_logging(app: &tauri::AppHandle) -> Result<(), ShelfError> {
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|e| ShelfError::Internal(format!("ログディレクトリの解決に失敗しました: {e}")))?;
    std::fs::create_dir_all(&log_dir)
        .map_err(|e| ShelfError::Internal(format!("ログディレクトリの作成に失敗しました: {e}")))?;

    let file_appender = tracing_appender::rolling::daily(&log_dir, "shelf-drop.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    // アプリ生存期間中ずっとログを書き続けたいため、非破棄ガードを意図的にリークさせる。
    // アプリプロセス終了時にOSへ回収されるため実質的な問題は無い。
    Box::leak(Box::new(guard));

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    Ok(())
}
