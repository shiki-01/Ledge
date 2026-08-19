//! アプリ全体で共有するドメインエラー型（architecture.md 6章）。
//!
//! Tauriコマンドの戻り値は`Result<T, ShelfError>`として扱い、失敗時はここでSerialize実装した
//! 形（コード＋ユーザー向けメッセージ）でそのままフロントエンドへシリアライズされる。

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ShelfError {
    #[error("データベースエラーが発生しました: {0}")]
    Database(String),

    #[error("指定されたアイテムが見つかりません: {0}")]
    NotFound(String),

    #[error("設定の読み書きに失敗しました: {0}")]
    Settings(String),

    #[error("グローバルショートカットの登録に失敗しました: {0}")]
    Shortcut(String),

    #[error("ドラッグ&ドロップ処理に失敗しました: {0}")]
    DragDropFailed(String),

    #[error("{0}")]
    Conflict(String),

    #[error("内部エラーが発生しました: {0}")]
    Internal(String),
}

/// フロントエンドへ渡す際のペイロード（コード＋メッセージ）。
#[derive(Debug, Serialize)]
struct ShelfErrorPayload {
    code: String,
    message: String,
}

impl ShelfError {
    fn code(&self) -> &'static str {
        match self {
            ShelfError::Database(_) => "database_error",
            ShelfError::NotFound(_) => "not_found",
            ShelfError::Settings(_) => "settings_error",
            ShelfError::Shortcut(_) => "shortcut_error",
            ShelfError::DragDropFailed(_) => "drag_drop_failed",
            ShelfError::Conflict(_) => "conflict",
            ShelfError::Internal(_) => "internal_error",
        }
    }
}

impl Serialize for ShelfError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ShelfErrorPayload {
            code: self.code().to_string(),
            message: self.to_string(),
        }
        .serialize(serializer)
    }
}
