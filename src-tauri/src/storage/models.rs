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

/// よく使うフォルダ（`favorite_folders`テーブルに対応、Phase6, F-09）。
///
/// `shelf_items`と異なり常時表示するブックマークのため独立テーブルとして持つ
/// （architecture.md 12.1章）。`missing`は`ShelfItem`と同様DBには保存せず、
/// 一覧取得のたびにディレクトリの存在チェックを行って算出する。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteFolder {
    pub id: i64,
    pub folder_path: String,
    pub display_name: String,
    pub sort_order: i64,
    pub added_at: String,
    pub missing: bool,
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
/// `content_hash`はもともと重複排除専用の内部キーとしてフロントエンドに公開していなかったが、
/// F-22（デバイス間同期）でFirestoreドキュメントIDとして`content_hash`をそのまま流用する設計
/// （architecture.md 10.2章）にしたため、同期エンジン（`src/lib/sync/clipboardSync.ts`）が
/// pushのdiff計算に使えるようこの構造体にも公開する（迷った設計判断: 呼び出し元へ報告。
/// TypeScript側でSHA-256アルゴリズムを再実装してRust側と同じハッシュ値を得る代替案もあったが、
/// 実装の二重化によるアルゴリズム不一致のリスクを避けるため、既存のDBカラムをそのまま
/// 公開する方を選んだ）。
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
    /// 重複排除・F-22同期のFirestoreドキュメントIDに使うcontent hash（上記コメント参照）。
    pub content_hash: String,
    /// このエントリに付与されたタグ（Phase4, F-17）。`clipboard_tags`とのJOIN結果を
    /// 一覧取得時にまとめて付与する（architecture.md 9.3章のコマンド一覧には無いが、
    /// フロント側のタグチップ表示・タグ付けUIのため一覧取得結果に含める設計にした。
    /// 迷った設計判断: 呼び出し元へ報告）。
    pub tags: Vec<Tag>,
}

/// タグ（`tags`テーブルに対応、Phase4, F-17）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: i64,
    pub name: String,
    /// 自由入力のhexカラーコード文字列（`#RRGGBB`）。DB制約は設けない（architecture.md 9.3章）。
    pub color: Option<String>,
}
