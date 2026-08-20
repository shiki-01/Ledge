//! よく使うフォルダ関連のTauriコマンド（architecture.md 12.1章、Phase6 F-09）。

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, State};

use crate::drag_drop;
use crate::error::ShelfError;
use crate::storage::favorite_folder_repo;
use crate::storage::models::FavoriteFolder;
use crate::AppState;

/// DB更新を伴うコマンド完了後にフロントへ通知するイベント名（architecture.md 12.1章）。
const EVENT_FAVORITES_CHANGED: &str = "favorites://changed";

#[tauri::command]
pub fn favorites_list(state: State<'_, AppState>) -> Result<Vec<FavoriteFolder>, ShelfError> {
    let conn = state.db.0.lock().map_err(lock_err)?;
    favorite_folder_repo::list_items(&conn)
}

#[tauri::command]
pub fn favorites_add(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<FavoriteFolder, ShelfError> {
    let added = {
        let conn = state.db.0.lock().map_err(lock_err)?;
        favorite_folder_repo::add(&conn, &path)?
    };
    notify_favorites_changed(&app);
    Ok(added)
}

#[tauri::command]
pub fn favorites_remove(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), ShelfError> {
    {
        let conn = state.db.0.lock().map_err(lock_err)?;
        favorite_folder_repo::remove(&conn, id)?;
    }
    notify_favorites_changed(&app);
    Ok(())
}

/// よく使うフォルダを外部アプリ/フォルダへネイティブドラッグで送り出す（F-03の`DragOutSource`を流用）。
#[tauri::command]
pub fn favorites_begin_drag_out(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), ShelfError> {
    let path = {
        let conn = state.db.0.lock().map_err(lock_err)?;
        favorite_folder_repo::get_path(&conn, id)?
    };

    let source = drag_drop::create_drag_out_source(app);
    source.begin_drag(vec![PathBuf::from(path)])
}

fn notify_favorites_changed(app: &AppHandle) {
    // フロントは購読して自動再取得するのみなので、送信失敗（購読者なし等）は無視してよい
    let _ = app.emit(EVENT_FAVORITES_CHANGED, ());
}

fn lock_err<T>(_: std::sync::PoisonError<T>) -> ShelfError {
    ShelfError::Internal("内部ロックの取得に失敗しました".into())
}
