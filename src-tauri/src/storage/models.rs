//! `shelf_items`テーブルに対応するRust構造体（Serialize/Deserialize）。
//! フィールド名はTypeScript側（src/lib/types/shelf.ts）とcamelCaseで揃える。

use serde::{Deserialize, Serialize};

/// シェルフアイテムの種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShelfItemType {
    File,
    Folder,
}

impl ShelfItemType {
    pub fn as_db_str(self) -> &'static str {
        match self {
            ShelfItemType::File => "file",
            ShelfItemType::Folder => "folder",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "folder" => ShelfItemType::Folder,
            _ => ShelfItemType::File,
        }
    }
}

/// シェルフ内の1アイテム（DBレコード＋実行時の存在チェック結果）。
///
/// `missing`はDBには保存せず、一覧取得のたびに`source_path`の存在チェックを行って
/// 算出する（requirements.md 10.1章: 元ファイルが削除された場合はmissing状態でグレーアウト表示する）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfItem {
    pub id: i64,
    pub item_type: ShelfItemType,
    pub source_path: String,
    pub display_name: String,
    pub size_bytes: Option<i64>,
    pub locked: bool,
    pub sort_order: i64,
    pub added_at: String,
    pub missing: bool,
    /// 元ファイルの最終更新日時（Unixエポックミリ秒）。DBには保存せず`missing`と同様に
    /// 一覧取得のたびにファイルシステムから算出する（F-07プレビュー表示用。取得できない場合は
    /// `None`）。フォーマットはフロント側（`Date`）に委ねる（迷った設計判断: Rust側に日付
    /// フォーマット用の依存を増やさないための選択）。
    pub modified_at_ms: Option<i64>,
}

/// クリップボード履歴アイテムの内容種別（Phase2, F-11）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardContentType {
    Text,
    Image,
    FilePaths,
}

impl ClipboardContentType {
    pub fn as_db_str(self) -> &'static str {
        match self {
            ClipboardContentType::Text => "text",
            ClipboardContentType::Image => "image",
            ClipboardContentType::FilePaths => "file_paths",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "image" => ClipboardContentType::Image,
            "file_paths" => ClipboardContentType::FilePaths,
            _ => ClipboardContentType::Text,
        }
    }
}

/// クリップボード履歴の1件（`clipboard_history`テーブルに対応、Phase2）。
///
/// `content_hash`は重複排除専用の内部キーであり、フロントエンドに公開する必要が無いため
/// この構造体には含めない（architecture.md 2章のDDLにはカラムとして存在する）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardEntry {
    pub id: i64,
    pub content_type: ClipboardContentType,
    pub text_content: Option<String>,
    pub image_path: Option<String>,
    pub thumbnail_path: Option<String>,
    /// `content_type == FilePaths`の場合のみ値を持つ（DB上は`file_paths_json`にJSON配列で保存）。
    pub file_paths: Option<Vec<String>>,
    pub pinned: bool,
    pub created_at: String,
    pub updated_at: String,
}
