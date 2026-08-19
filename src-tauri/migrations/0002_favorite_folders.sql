-- architecture.md 12.1章のDDLに対応するマイグレーション（Phase6, F-09）。
-- よく使うフォルダ（ショートカット）を常時表示するための専用テーブル。
-- shelf_itemsとはライフサイクルが異なる（ドラッグ&ドロップの都度追加・「全て削除」で消える
-- 一時置き場ではなく、常時表示するブックマーク）ため、shelf_itemsを流用せず新規テーブルとする。

CREATE TABLE favorite_folders (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    folder_path   TEXT NOT NULL UNIQUE,  -- 同一フォルダの重複登録を防ぐ（shelf_itemsと異なり「ブックマーク」なので重複を許さない）
    display_name  TEXT NOT NULL,
    sort_order    INTEGER NOT NULL,
    added_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
