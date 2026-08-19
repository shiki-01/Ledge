//! クリップボード関連のTauriコマンド（architecture.md 3章 Phase2表）。

use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{debug, error};

use crate::clipboard::ClipboardSnapshot;
use crate::error::ShelfError;
use crate::settings;
use crate::storage::clipboard_repo;
use crate::storage::models::{ClipboardContentType, ClipboardEntry};
use crate::AppState;

/// DB更新を伴うコマンド完了後にフロントへ通知するイベント名（architecture.md 3章）。
const EVENT_HISTORY_CHANGED: &str = "clipboard://history-changed";

#[tauri::command]
pub fn clipboard_list_history(
    state: State<'_, AppState>,
    query: Option<String>,
) -> Result<Vec<ClipboardEntry>, ShelfError> {
    let conn = state.db.0.lock().map_err(lock_err)?;
    clipboard_repo::list_history(&conn, query.as_deref())
}

/// 履歴アイテムをクリップボードへ書き戻す（F-12）。
///
/// 書き込みには`arboard`を使う（読み取り監視はOS別実装、書き込みは`arboard`という役割分担、
/// requirements.md 5章）。書き込み直前にcontent hashを`SelfWriteGuard`へ`mark`しておくことで、
/// 直後の監視イベントによる再記録（自己ループ）を防ぐ（requirements.md 10.2章）。
#[tauri::command]
pub fn clipboard_paste_to_active(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<(), ShelfError> {
    let entry = {
        let conn = state.db.0.lock().map_err(lock_err)?;
        clipboard_repo::get_entry(&conn, id)?
    };

    let mut clipboard = arboard::Clipboard::new().map_err(clipboard_err)?;

    match entry.content_type {
        ClipboardContentType::Text => {
            let text = entry.text_content.unwrap_or_default();
            let hash = clipboard_repo::content_hash(&ClipboardSnapshot::Text(text.clone()));
            state.clipboard_guard.mark(hash);
            clipboard.set_text(text).map_err(clipboard_err)?;
        }
        ClipboardContentType::Image => {
            let image_path = entry
                .image_path
                .ok_or_else(|| ShelfError::Internal("画像パスが記録されていません".into()))?;
            let png_bytes = std::fs::read(&image_path)
                .map_err(|e| ShelfError::Internal(format!("画像キャッシュの読み込みに失敗しました: {e}")))?;

            let hash = clipboard_repo::content_hash(&ClipboardSnapshot::Image(png_bytes.clone()));
            state.clipboard_guard.mark(hash);

            let decoded = image::load_from_memory(&png_bytes)
                .map_err(|e| ShelfError::Internal(format!("画像のデコードに失敗しました: {e}")))?
                .to_rgba8();
            let (width, height) = decoded.dimensions();
            let image_data = arboard::ImageData {
                width: width as usize,
                height: height as usize,
                bytes: std::borrow::Cow::Owned(decoded.into_raw()),
            };
            clipboard.set_image(image_data).map_err(clipboard_err)?;
        }
        ClipboardContentType::FilePaths => {
            // arboardはOS間で統一されたファイルリスト形式（CF_HDROP / NSFilenamesPboardType）の
            // 書き込みAPIを提供していないため、Phase2では未対応とする。改行区切りテキストとしての
            // フォールバックはしない（Explorer/Finderへの「貼り付け」を期待するユーザー体験を
            // 壊すため）。将来的にOS別の直接実装が必要になれば`drag_drop`と同様trait化を検討する
            // （呼び出し元への報告事項: 迷った設計判断）。
            return Err(ShelfError::Internal(
                "ファイルパスのクリップボードへの書き戻しは現在対応していません".into(),
            ));
        }
    }

    notify_history_changed(&app);
    Ok(())
}

/// ピン留め状態を変更する（F-13）。
#[tauri::command]
pub fn clipboard_set_pinned(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    pinned: bool,
) -> Result<(), ShelfError> {
    {
        let conn = state.db.0.lock().map_err(lock_err)?;
        clipboard_repo::set_pinned(&conn, id, pinned)?;
    }
    notify_history_changed(&app);
    Ok(())
}

/// 個別削除。
#[tauri::command]
pub fn clipboard_delete(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<(), ShelfError> {
    {
        let conn = state.db.0.lock().map_err(lock_err)?;
        clipboard_repo::delete_entry(&conn, id)?;
    }
    notify_history_changed(&app);
    Ok(())
}

/// 一括削除（F-16の手動版に相当。自動クリアは`handle_clipboard_change`から呼ばれる）。
#[tauri::command]
pub fn clipboard_clear(
    app: AppHandle,
    state: State<'_, AppState>,
    exclude_pinned: bool,
) -> Result<(), ShelfError> {
    {
        let conn = state.db.0.lock().map_err(lock_err)?;
        clipboard_repo::clear(&conn, exclude_pinned)?;
    }
    notify_history_changed(&app);
    Ok(())
}

/// `clipboard::ClipboardWatcher`の変更検知コールバックから呼ばれる（`lib.rs`のsetupで配線）。
/// フロントから直接invokeされる`#[tauri::command]`ではないため、失敗してもエラーはログに残すのみで
/// 呼び出し元（監視スレッド）へは伝播させない。
pub(crate) fn handle_clipboard_change(app: &AppHandle, snapshot: ClipboardSnapshot) {
    let state = app.state::<AppState>();
    let hash = clipboard_repo::content_hash(&snapshot);

    if state.clipboard_guard.consume_if_matches(&hash) {
        debug!("自己書き込みによる変更のため、クリップボード履歴への記録をスキップしました");
        return;
    }

    let settings = match settings::load_settings(app) {
        Ok(settings) => settings,
        Err(e) => {
            error!(error = %e, "設定の読み込みに失敗したためクリップボード履歴の記録を中止しました");
            return;
        }
    };

    let result = (|| -> Result<(), ShelfError> {
        let conn = state.db.0.lock().map_err(lock_err)?;
        clipboard_repo::record_entry(&conn, &state.clipboard_cache_dir, &snapshot, &hash)?;
        clipboard_repo::enforce_retention(&conn, settings.clipboard_max_entries, settings.clipboard_retention_days)?;
        Ok(())
    })();

    match result {
        Ok(()) => notify_history_changed(app),
        Err(e) => error!(error = %e, "クリップボード履歴の記録に失敗しました"),
    }
}

fn notify_history_changed(app: &AppHandle) {
    // フロントは購読して自動再取得するのみなので、送信失敗（購読者なし等）は無視してよい
    let _ = app.emit(EVENT_HISTORY_CHANGED, ());
}

fn clipboard_err(e: arboard::Error) -> ShelfError {
    ShelfError::Internal(format!("クリップボードへのアクセスに失敗しました: {e}"))
}

fn lock_err<T>(_: std::sync::PoisonError<T>) -> ShelfError {
    ShelfError::Internal("内部ロックの取得に失敗しました".into())
}
