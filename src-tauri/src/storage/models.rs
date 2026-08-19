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
}
