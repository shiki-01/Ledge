//! タグ関連のTauriコマンド（architecture.md 9.3章）。
//!
//! `clipboard_set_tags`はエントリ側の操作だが、タグ管理コマンド群とまとめて置いた方が
//! 見通しが良いと判断しこのファイルに置いている（`commands::clipboard`に置く選択肢もあった、
//! 迷った設計判断: 呼び出し元へ報告）。

use tauri::{AppHandle, Emitter, State};

use crate::commands::clipboard::notify_history_changed;
use crate::error::ShelfError;
use crate::storage::models::Tag;
use crate::storage::tags_repo;
use crate::AppState;

/// タグ一覧・タグ付けが変わった際にフロントへ通知するイベント名。
/// `clipboard://history-changed`とは別に用意し、タグ管理UI（タグ一覧の増減）と
/// クリップボード一覧（エントリごとのタグ付け表示）を独立して再取得できるようにする。
const EVENT_TAGS_CHANGED: &str = "tags://changed";

/// タグ一覧を取得する。
#[tauri::command]
pub fn tags_list(state: State<'_, AppState>) -> Result<Vec<Tag>, ShelfError> {
    let conn = state.db.0.lock().map_err(lock_err)?;
    tags_repo::list_tags(&conn)
}

/// タグを作成する。`name`のUNIQUE制約違反は`ShelfError::Conflict`として返る。
#[tauri::command]
pub fn tags_create(app: AppHandle, state: State<'_, AppState>, name: String, color: Option<String>) -> Result<Tag, ShelfError> {
    let tag = {
        let conn = state.db.0.lock().map_err(lock_err)?;
        tags_repo::create_tag(&conn, &name, color.as_deref())?
    };
    notify_tags_changed(&app);
    Ok(tag)
}

/// タグを削除する。`clipboard_tags`はON DELETE CASCADEのため関連付けも自動的に消える。
/// タグ付け表示が変わるクリップボード一覧側にも合わせて通知する。
#[tauri::command]
pub fn tags_delete(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<(), ShelfError> {
    {
        let conn = state.db.0.lock().map_err(lock_err)?;
        tags_repo::delete_tag(&conn, id)?;
    }
    notify_tags_changed(&app);
    notify_history_changed(&app);
    Ok(())
}

/// 指定エントリのタグ付けを一括置き換えする（F-17）。
#[tauri::command]
pub fn clipboard_set_tags(app: AppHandle, state: State<'_, AppState>, id: i64, tag_ids: Vec<i64>) -> Result<(), ShelfError> {
    {
        let conn = state.db.0.lock().map_err(lock_err)?;
        tags_repo::set_clipboard_tags(&conn, id, &tag_ids)?;
    }
    notify_history_changed(&app);
    Ok(())
}

fn notify_tags_changed(app: &AppHandle) {
    let _ = app.emit(EVENT_TAGS_CHANGED, ());
}

fn lock_err<T>(_: std::sync::PoisonError<T>) -> ShelfError {
    ShelfError::Internal("内部ロックの取得に失敗しました".into())
}
