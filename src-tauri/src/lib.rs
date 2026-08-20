//! Tauri Builderの組み立て、プラグイン登録、`run()`のエントリポイント。
//! 実際の起動処理は`main.rs`から`run()`を呼ぶだけの薄い委譲構成にする（architecture.md 1章）。

pub mod clipboard;
pub mod commands;
pub mod compress;
pub mod drag_drop;
pub mod drag_watch;
pub mod error;
pub mod settings;
pub mod shortcut;
pub mod storage;
pub mod tray;
pub mod window;

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::Manager;
use tracing_subscriber::EnvFilter;

use error::ShelfError;
use storage::db::Db;

/// アプリ全体で共有する状態。`tauri::State`経由で各コマンドから参照する。
pub struct AppState {
    pub db: Db,
    /// クリップボード履歴の自己ループ防止ガード（requirements.md 10.2章、clipboard/mod.rs参照）。
    pub clipboard_guard: clipboard::SelfWriteGuard,
    /// クリップボード画像キャッシュの保存先（`app_data_dir`配下、requirements.md 10.2章）。
    pub clipboard_cache_dir: PathBuf,
    /// ドラッグ開始検知（F-08 Windows先行）の監視インスタンス。設定変更時に`stop()`/`start()`
    /// できるよう、clipboard watcherと異なりリークさせずMutexで保持する。
    pub drag_watcher: Mutex<Box<dyn drag_watch::DragWatcher>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        // 多重起動防止（architecture.md 5章）。2重起動時は既存インスタンスのシェルフを表示する。
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            window::show_shelf(app);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::new().build())
        // F-19（自動起動）。Windows/macOS/Linuxいずれでも動作するため、target cfgでは絞り込まない。
        // macOS向けのlauncher種別はLaunchAgent方式を採用する（もう一方の選択肢はAppleScript経由）。
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        // F-09（よく使うフォルダ登録）: フォルダ選択ダイアログ。公式・クロスプラットフォーム対応。
        .plugin(tauri_plugin_dialog::init())
        // F-21（右クリックメニュー）:「エクスプローラー/Finderで表示」用。公式・クロスプラットフォーム対応。
        .plugin(tauri_plugin_opener::init());

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
            let clipboard_cache_dir = resolve_clipboard_cache_dir(&app_handle)?;

            let settings = settings::load_settings(&app_handle)?;

            // ドラッグ開始検知（F-08 Windows先行）。設定でON/OFFできるため、Mutexで保持して
            // 後から`drag_watch::set_enabled`経由でstart/stopできるようにする
            // （clipboard watcherと異なりBox::leakしない、迷った設計判断: 呼び出し元へ報告）。
            let mut drag_watcher = drag_watch::create_watcher();
            if settings.auto_show_on_drag_start {
                let start_handle = app_handle.clone();
                let end_handle = app_handle.clone();
                let edge = drag_watch::compute_edge_geometry(&app_handle);
                drag_watcher.start(
                    Box::new(move || window::show_shelf_auto(&start_handle)),
                    Box::new(move || window::hide_shelf_if_auto(&end_handle)),
                    edge,
                )?;
            }

            // 既存のシェルフアイテム（前回起動時に追加されたもの）についても、F-07プレビューの
            // asset protocol読み込みに必要な分だけスコープを許可しておく（新規追加分は
            // `commands::shelf::grant_preview_scope`で都度許可する。tauri.conf.jsonでは
            // "**" のような包括スコープを敢えて置かず、実際に参照するパスだけを動的に許可する
            // 設計にしている）。
            {
                let conn = db.0.lock().map_err(|_| ShelfError::Internal("内部ロックの取得に失敗しました".into()))?;
                let existing_items = storage::shelf_repo::list_items(&conn)?;
                drop(conn);
                commands::shelf::grant_preview_scope(&app_handle, &existing_items);
            }

            app.manage(AppState {
                db,
                clipboard_guard: clipboard::SelfWriteGuard::new(),
                clipboard_cache_dir,
                drag_watcher: std::sync::Mutex::new(drag_watcher),
            });

            tray::setup_tray(&app_handle)?;

            shortcut::register_shortcuts(&app_handle, &settings)?;

            // クリップボード監視を開始する（F-11）。監視スレッドは検知のたびに
            // `commands::clipboard::handle_clipboard_change`を呼び、DBへの記録・イベント通知を行う。
            // watcher自体はアプリの生存期間中ずっと動き続けてよいため、tracingのガードと同様に
            // リークさせて保持する（architecture.md 5章の「多重起動防止」等と異なり、明示的な
            // stop()呼び出しの必要が今のところ無いための単純化。呼び出し元への報告事項:
            // 迷った設計判断）。
            let mut clipboard_watcher = clipboard::create_watcher();
            let watcher_app_handle = app_handle.clone();
            clipboard_watcher.start(Box::new(move |snapshot| {
                commands::clipboard::handle_clipboard_change(&watcher_app_handle, snapshot);
            }))?;
            Box::leak(clipboard_watcher);

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
            commands::shelf::shelf_set_locked,
            commands::shelf::shelf_clear,
            commands::shelf::shelf_begin_drag_out,
            commands::shelf::shelf_copy_path,
            commands::shelf::shelf_compress_item,
            commands::favorites::favorites_list,
            commands::favorites::favorites_add,
            commands::favorites::favorites_remove,
            commands::favorites::favorites_begin_drag_out,
            commands::clipboard::clipboard_list_history,
            commands::clipboard::clipboard_paste_to_active,
            commands::clipboard::clipboard_set_pinned,
            commands::clipboard::clipboard_delete,
            commands::clipboard::clipboard_clear,
            commands::clipboard::clipboard_stack_entries,
            commands::clipboard::clipboard_sync_upsert_from_cloud,
            commands::clipboard::clipboard_sync_unpin_by_hash,
            commands::tags::tags_list,
            commands::tags::tags_create,
            commands::tags::tags_delete,
            commands::tags::clipboard_set_tags,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::sync_set_firebase_password,
        ])
        .run(tauri::generate_context!())
        .expect("Ledge の起動に失敗しました");
}

/// SQLiteファイルの保存先（アプリデータディレクトリ配下）を解決する。
fn resolve_db_path(app: &tauri::AppHandle) -> Result<PathBuf, ShelfError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| ShelfError::Internal(format!("アプリデータディレクトリの解決に失敗しました: {e}")))?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| ShelfError::Internal(format!("アプリデータディレクトリの作成に失敗しました: {e}")))?;
    Ok(dir.join("ledge.sqlite3"))
}

/// クリップボード画像キャッシュ（PNG）の保存先ディレクトリを解決する（requirements.md 10.2章）。
/// 実際のディレクトリ作成は初回の画像記録時（`clipboard_repo::record_entry`）まで遅延させる。
fn resolve_clipboard_cache_dir(app: &tauri::AppHandle) -> Result<PathBuf, ShelfError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| ShelfError::Internal(format!("アプリデータディレクトリの解決に失敗しました: {e}")))?;
    Ok(dir.join("clipboard-cache"))
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

    let file_appender = tracing_appender::rolling::daily(&log_dir, "ledge.log");
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
