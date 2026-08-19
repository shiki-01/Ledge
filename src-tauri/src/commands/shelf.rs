//! シェルフ関連のTauriコマンド（architecture.md 3章 Phase1表）。

use tauri::{AppHandle, Emitter, State};

use crate::drag_drop;
use crate::error::ShelfError;
use crate::storage::models::ShelfItem;
use crate::storage::shelf_repo;
use crate::AppState;

/// DB更新を伴うコマンド完了後にフロントへ通知するイベント名（architecture.md 3章）。
const EVENT_ITEMS_CHANGED: &str = "shelf://items-changed";

#[tauri::command]
pub fn shelf_list_items(state: State<'_, AppState>) -> Result<Vec<ShelfItem>, ShelfError> {
    let conn = state.db.0.lock().map_err(lock_err)?;
    shelf_repo::list_items(&conn)
}

#[tauri::command]
pub fn shelf_add_paths(
    app: AppHandle,
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<Vec<ShelfItem>, ShelfError> {
    let added = {
        let conn = state.db.0.lock().map_err(lock_err)?;
        shelf_repo::add_paths(&conn, &paths)?
    };
    notify_items_changed(&app);
    Ok(added)
}

#[tauri::command]
pub fn shelf_remove_item(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), ShelfError> {
    {
        let conn = state.db.0.lock().map_err(lock_err)?;
        shelf_repo::remove_item(&conn, id)?;
    }
    notify_items_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn shelf_clear(
    app: AppHandle,
    state: State<'_, AppState>,
    exclude_locked: bool,
) -> Result<(), ShelfError> {
    {
        let conn = state.db.0.lock().map_err(lock_err)?;
        shelf_repo::clear(&conn, exclude_locked)?;
    }
    notify_items_changed(&app);
    Ok(())
}

/// シェルフ内アイテムを外部アプリ/フォルダへネイティブドラッグで送り出す（F-03）。
#[tauri::command]
pub fn shelf_begin_drag_out(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<i64>,
) -> Result<(), ShelfError> {
    let paths = {
        let conn = state.db.0.lock().map_err(lock_err)?;
        shelf_repo::get_paths(&conn, &ids)?
    };
    let paths = paths.into_iter().map(std::path::PathBuf::from).collect();

    let source = drag_drop::create_drag_out_source(app);
    source.begin_drag(paths)
}

fn notify_items_changed(app: &AppHandle) {
    // フロントは購読して自動再取得するのみなので、送信失敗（購読者なし等）は無視してよい
    let _ = app.emit(EVENT_ITEMS_CHANGED, ());
}

fn lock_err<T>(_: std::sync::PoisonError<T>) -> ShelfError {
    ShelfError::Internal("内部ロックの取得に失敗しました".into())
}
