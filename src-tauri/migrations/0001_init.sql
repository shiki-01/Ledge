-- architecture.md 2章のDDLに対応する初期マイグレーション。
-- Phase2以降で使うテーブルも先行して作成しておくが、Phase1のコードからは
-- shelf_itemsのみを使用する（architecture.md 8章の方針）。

-- シェルフアイテム（Phase1〜）
CREATE TABLE shelf_items (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    item_type     TEXT NOT NULL CHECK (item_type IN ('file', 'folder')),
    source_path   TEXT NOT NULL,
    display_name  TEXT NOT NULL,
    size_bytes    INTEGER,
    locked        INTEGER NOT NULL DEFAULT 0,   -- F-06 (Phase3)
    sort_order    INTEGER NOT NULL,
    added_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_shelf_items_sort_order ON shelf_items(sort_order);

-- クリップボード履歴（Phase2〜）
CREATE TABLE clipboard_history (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    content_type    TEXT NOT NULL CHECK (content_type IN ('text', 'image', 'file_paths')),
    text_content    TEXT,
    image_path      TEXT,          -- アプリデータディレクトリ配下のPNGファイルパス（BLOBはDBに入れない）
    thumbnail_path  TEXT,
    file_paths_json TEXT,          -- content_type='file_paths'の場合のJSON配列
    content_hash    TEXT NOT NULL, -- 重複排除キー
    pinned          INTEGER NOT NULL DEFAULT 0,  -- F-13 (Phase2)
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE UNIQUE INDEX idx_clipboard_content_hash ON clipboard_history(content_hash);
CREATE INDEX idx_clipboard_created_at ON clipboard_history(created_at);

-- タグ（Phase4, F-17）
CREATE TABLE tags (
    id    INTEGER PRIMARY KEY AUTOINCREMENT,
    name  TEXT NOT NULL UNIQUE,
    color TEXT
);
CREATE TABLE clipboard_tags (
    clipboard_id INTEGER NOT NULL REFERENCES clipboard_history(id) ON DELETE CASCADE,
    tag_id       INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (clipboard_id, tag_id)
);

-- スタック（Phase4, F-15）: クリップボード項目の結合はDB上は独立エンティティとして新規text履歴を1件作る方式とし、
-- 専用テーブルは持たない（結合＝新しいテキストアイテムの生成、という単純なモデルにする）
