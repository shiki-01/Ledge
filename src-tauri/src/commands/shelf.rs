//! シェルフ関連のTauriコマンド（architecture.md 3章 Phase1表）。

use tauri::{AppHandle, Emitter, Manager, State};

use crate::compress;
use crate::drag_drop;
use crate::error::ShelfError;
use crate::storage::models::{ShelfItem, ShelfItemType};
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
    grant_preview_scope(&app, &added);
    notify_items_changed(&app);
    Ok(added)
}

/// F-07のプレビュー表示（asset protocol経由の画像読み込み）に必要な分だけ、シェルフアイテムの
/// 実パスをasset protocolの許可スコープへ動的に追加する。
///
/// シェルフはユーザーがドラッグした任意のパスを参照する（`docs/requirements.md` 10.1章）ため
/// 事前の静的allowlistでは絞り込めないが、`tauri.conf.json`に`"**"`のような包括的スコープを
/// 置くとwebviewから任意のローカルファイルパスを読めてしまい攻撃対象範囲が不必要に広がる。
/// そのため実際にシェルフへ追加された実ファイルのパスのみを都度許可する（フォルダはプレビュー
/// 対象外のため許可しない）。失敗しても致命的ではないため警告ログに留める。
pub(crate) fn grant_preview_scope(app: &AppHandle, items: &[ShelfItem]) {
    let scope = app.asset_protocol_scope();
    for item in items {
        if item.item_type != ShelfItemType::File {
            continue;
        }
        if let Err(e) = scope.allow_file(&item.source_path) {
            tracing::warn!(path = %item.source_path, error = %e, "asset protocolスコープへの許可追加に失敗しました");
        }
    }
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

/// ロック状態の変更（F-06）。
#[tauri::command]
pub fn shelf_set_locked(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    locked: bool,
) -> Result<(), ShelfError> {
    {
        let conn = state.db.0.lock().map_err(lock_err)?;
        shelf_repo::set_locked(&conn, id, locked)?;
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

/// パスをクリップボードへコピーする（F-21右クリックメニュー）。
///
/// `clipboard_paste_to_active`と異なり`clipboard_guard`はかけない。通常の外部コピーと同様に
/// クリップボード履歴（F-11）へ自然に記録されてよい、という判断（architecture.md 12.2章）。
#[tauri::command]
pub fn shelf_copy_path(state: State<'_, AppState>, id: i64) -> Result<(), ShelfError> {
    let item = {
        let conn = state.db.0.lock().map_err(lock_err)?;
        shelf_repo::get_item(&conn, id)?
    };

    let mut clipboard = arboard::Clipboard::new().map_err(clipboard_err)?;
    clipboard
        .set_text(item.source_path)
        .map_err(clipboard_err)?;
    Ok(())
}

/// 対象アイテムをZIP圧縮し、新規シェルフアイテムとして追加する（F-21右クリックメニュー）。
///
/// 圧縮処理自体はDBアクセスを伴わない（`compress`モジュール参照）ため、フォルダが大きい場合に
/// DBロックを長時間保持しないよう、取得・圧縮・追加でそれぞれ個別にロックを取得する
/// （迷った設計判断: 呼び出し元へ報告）。
#[tauri::command]
pub fn shelf_compress_item(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<ShelfItem, ShelfError> {
    let target = {
        let conn = state.db.0.lock().map_err(lock_err)?;
        shelf_repo::get_item(&conn, id)?
    };

    let zip_path = compress::compress_to_zip(
        std::path::Path::new(&target.source_path),
        &target.display_name,
    )?;
    let zip_path_str = zip_path.to_string_lossy().to_string();

    let added = {
        let conn = state.db.0.lock().map_err(lock_err)?;
        let mut added = shelf_repo::add_paths(&conn, &[zip_path_str])?;
        added.pop().ok_or_else(|| {
            ShelfError::Internal("圧縮後のシェルフアイテム追加に失敗しました".into())
        })?
    };
    grant_preview_scope(&app, std::slice::from_ref(&added));
    notify_items_changed(&app);
    Ok(added)
}

fn clipboard_err(e: arboard::Error) -> ShelfError {
    ShelfError::Internal(format!("クリップボードへのアクセスに失敗しました: {e}"))
}

fn notify_items_changed(app: &AppHandle) {
    // フロントは購読して自動再取得するのみなので、送信失敗（購読者なし等）は無視してよい
    let _ = app.emit(EVENT_ITEMS_CHANGED, ());
}

fn lock_err<T>(_: std::sync::PoisonError<T>) -> ShelfError {
    ShelfError::Internal("内部ロックの取得に失敗しました".into())
}
