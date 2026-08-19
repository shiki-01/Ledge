//! `clipboard_history`テーブルへのCRUD操作（Phase2, F-11〜F-13, F-16）。SQL文はこのファイルに閉じ込める。
//!
//! 画像はDBにBLOB格納せず、呼び出し元から渡されるキャッシュディレクトリ配下にPNGファイルとして
//! 保存し、パスのみDBに記録する（requirements.md 10.2章）。

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::clipboard::ClipboardSnapshot;
use crate::error::ShelfError;

use super::models::{ClipboardContentType, ClipboardEntry};

/// 重複排除・自己ループ防止の両方で使うcontent hashを計算する（requirements.md 10.2章）。
/// テキストは前後空白を取り除いた内容、画像はPNGバイト列そのもの、ファイルパスは
/// 改行区切りで結合した文字列をそれぞれSHA-256でハッシュ化する。
pub fn content_hash(snapshot: &ClipboardSnapshot) -> String {
    let mut hasher = Sha256::new();
    match snapshot {
        ClipboardSnapshot::Text(text) => {
            hasher.update(b"text:");
            hasher.update(text.trim().as_bytes());
        }
        ClipboardSnapshot::Image(bytes) => {
            hasher.update(b"image:");
            hasher.update(bytes);
        }
        ClipboardSnapshot::FilePaths(paths) => {
            hasher.update(b"file_paths:");
            for path in paths {
                hasher.update(path.to_string_lossy().as_bytes());
                hasher.update(b"\n");
            }
        }
    }
    format!("{:x}", hasher.finalize())
}

/// 履歴一覧を取得する。`query`が指定されている場合はテキスト内容/ファイルパスに対する
/// 簡易LIKE検索を行う（F-14の本格実装はPhase4だが、コマンド引数としては先行して受け付ける）。
/// ピン留めアイテムを先頭にまとめ、その中・それ以外それぞれ更新日時降順で並べる。
pub fn list_history(conn: &Connection, query: Option<&str>) -> Result<Vec<ClipboardEntry>, ShelfError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, content_type, text_content, image_path, thumbnail_path, file_paths_json, \
                    pinned, created_at, updated_at \
             FROM clipboard_history \
             WHERE (?1 IS NULL OR text_content LIKE '%' || ?1 || '%' OR file_paths_json LIKE '%' || ?1 || '%') \
             ORDER BY pinned DESC, updated_at DESC, id DESC",
        )
        .map_err(db_err)?;

    let rows = stmt
        .query_map(params![query], |row| {
            Ok(RawRow {
                id: row.get(0)?,
                content_type: row.get(1)?,
                text_content: row.get(2)?,
                image_path: row.get(3)?,
                thumbnail_path: row.get(4)?,
                file_paths_json: row.get(5)?,
                pinned: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })
        .map_err(db_err)?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.map_err(db_err)?.into_entry()?);
    }
    Ok(entries)
}

/// idを指定して1件取得する（F-12: 履歴からの貼り付け用）。
pub fn get_entry(conn: &Connection, id: i64) -> Result<ClipboardEntry, ShelfError> {
    let row = conn
        .query_row(
            "SELECT id, content_type, text_content, image_path, thumbnail_path, file_paths_json, \
                    pinned, created_at, updated_at \
             FROM clipboard_history WHERE id = ?1",
            params![id],
            |row| {
                Ok(RawRow {
                    id: row.get(0)?,
                    content_type: row.get(1)?,
                    text_content: row.get(2)?,
                    image_path: row.get(3)?,
                    thumbnail_path: row.get(4)?,
                    file_paths_json: row.get(5)?,
                    pinned: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            },
        )
        .optional()
        .map_err(db_err)?;

    match row {
        Some(row) => row.into_entry(),
        None => Err(ShelfError::NotFound(format!("clipboard entry id={id}"))),
    }
}

/// 監視で検知したクリップボードの内容を記録する。
///
/// 同一`hash`の行が既に存在する場合は新規行を作らず、タイムスタンプを更新して先頭に
/// 繰り上げる（ピン留め状態は保持、requirements.md 10.2章）。画像はキャッシュディレクトリへ
/// `{hash}.png`として保存し、パスのみDBに記録する。
pub fn record_entry(
    conn: &Connection,
    cache_dir: &Path,
    snapshot: &ClipboardSnapshot,
    hash: &str,
) -> Result<(), ShelfError> {
    let existing_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM clipboard_history WHERE content_hash = ?1",
            params![hash],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_err)?;

    if let Some(id) = existing_id {
        conn.execute(
            "UPDATE clipboard_history SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
            params![id],
        )
        .map_err(db_err)?;
        return Ok(());
    }

    let (content_type, text_content, image_path, file_paths_json) = match snapshot {
        ClipboardSnapshot::Text(text) => (ClipboardContentType::Text, Some(text.clone()), None, None),
        ClipboardSnapshot::Image(bytes) => {
            std::fs::create_dir_all(cache_dir)
                .map_err(|e| ShelfError::Internal(format!("画像キャッシュディレクトリの作成に失敗しました: {e}")))?;
            let path = cache_dir.join(format!("{hash}.png"));
            std::fs::write(&path, bytes)
                .map_err(|e| ShelfError::Internal(format!("画像キャッシュの書き込みに失敗しました: {e}")))?;
            (ClipboardContentType::Image, None, Some(path.to_string_lossy().to_string()), None)
        }
        ClipboardSnapshot::FilePaths(paths) => {
            let json_paths: Vec<String> = paths.iter().map(|p| p.to_string_lossy().to_string()).collect();
            let json = serde_json::to_string(&json_paths).map_err(|e| ShelfError::Database(e.to_string()))?;
            (ClipboardContentType::FilePaths, None, None, Some(json))
        }
    };

    // サムネイルは当面フル画像を流用する（専用サムネイル生成はPhase2スコープ外の簡略化）。
    let thumbnail_path = image_path.clone();

    conn.execute(
        "INSERT INTO clipboard_history \
            (content_type, text_content, image_path, thumbnail_path, file_paths_json, content_hash) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            content_type.as_db_str(),
            text_content,
            image_path,
            thumbnail_path,
            file_paths_json,
            hash
        ],
    )
    .map_err(db_err)?;

    Ok(())
}

/// ピン留め状態を変更する（F-13）。
pub fn set_pinned(conn: &Connection, id: i64, pinned: bool) -> Result<(), ShelfError> {
    let affected = conn
        .execute(
            "UPDATE clipboard_history SET pinned = ?1 WHERE id = ?2",
            params![pinned as i64, id],
        )
        .map_err(db_err)?;
    if affected == 0 {
        return Err(ShelfError::NotFound(format!("clipboard entry id={id}")));
    }
    Ok(())
}

/// 個別削除。画像キャッシュファイルも同時に削除する。
pub fn delete_entry(conn: &Connection, id: i64) -> Result<(), ShelfError> {
    let targets = collect_targets(conn, "SELECT id, image_path FROM clipboard_history WHERE id = ?1", params![id])?;
    if targets.is_empty() {
        return Err(ShelfError::NotFound(format!("clipboard entry id={id}")));
    }
    delete_targets(conn, &targets)
}

/// 一括削除（`excludePinned`が真の場合はピン留めアイテムを除外）。
pub fn clear(conn: &Connection, exclude_pinned: bool) -> Result<(), ShelfError> {
    let sql = if exclude_pinned {
        "SELECT id, image_path FROM clipboard_history WHERE pinned = 0"
    } else {
        "SELECT id, image_path FROM clipboard_history"
    };
    let targets = collect_targets(conn, sql, [])?;
    delete_targets(conn, &targets)
}

/// 自動クリア（F-16）: 経過日数超過分・件数上限超過分（いずれもピン留め除く）を削除する。
/// 画像キャッシュファイルも同時に削除する。
pub fn enforce_retention(conn: &Connection, max_entries: u32, retention_days: u32) -> Result<(), ShelfError> {
    delete_stale_by_age(conn, retention_days)?;
    delete_stale_by_count(conn, max_entries)?;
    Ok(())
}

fn delete_stale_by_age(conn: &Connection, retention_days: u32) -> Result<(), ShelfError> {
    let cutoff = format!("-{retention_days} days");
    let targets = collect_targets(
        conn,
        "SELECT id, image_path FROM clipboard_history \
         WHERE pinned = 0 AND created_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?1)",
        params![cutoff],
    )?;
    delete_targets(conn, &targets)
}

fn delete_stale_by_count(conn: &Connection, max_entries: u32) -> Result<(), ShelfError> {
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM clipboard_history", [], |row| row.get(0))
        .map_err(db_err)?;
    let max_entries = max_entries as i64;
    if total <= max_entries {
        return Ok(());
    }
    let overflow = total - max_entries;
    let targets = collect_targets(
        conn,
        "SELECT id, image_path FROM clipboard_history WHERE pinned = 0 ORDER BY updated_at ASC, id ASC LIMIT ?1",
        params![overflow],
    )?;
    delete_targets(conn, &targets)
}

/// 削除対象の(id, image_path)一覧を取得する共通ヘルパー。
fn collect_targets(
    conn: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
) -> Result<Vec<(i64, Option<String>)>, ShelfError> {
    let mut stmt = conn.prepare(sql).map_err(db_err)?;
    let rows = stmt
        .query_map(params, |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(db_err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

/// (id, image_path)一覧をDBから削除し、対応する画像キャッシュファイルも削除する。
fn delete_targets(conn: &Connection, targets: &[(i64, Option<String>)]) -> Result<(), ShelfError> {
    for (id, image_path) in targets {
        conn.execute("DELETE FROM clipboard_history WHERE id = ?1", params![id])
            .map_err(db_err)?;
        if let Some(path) = image_path {
            if let Err(e) = std::fs::remove_file(path) {
                // 既に無い場合等は握りつぶしてよいが、ログには残す
                tracing::warn!(path = %path, error = %e, "画像キャッシュファイルの削除に失敗しました");
            }
        }
    }
    Ok(())
}

/// SELECTの生の行。JSON展開等はここでは行わず、`into_entry`で行う。
struct RawRow {
    id: i64,
    content_type: String,
    text_content: Option<String>,
    image_path: Option<String>,
    thumbnail_path: Option<String>,
    file_paths_json: Option<String>,
    pinned: i64,
    created_at: String,
    updated_at: String,
}

impl RawRow {
    fn into_entry(self) -> Result<ClipboardEntry, ShelfError> {
        let file_paths = match self.file_paths_json {
            Some(json) => Some(
                serde_json::from_str::<Vec<String>>(&json).map_err(|e| ShelfError::Database(e.to_string()))?,
            ),
            None => None,
        };
        Ok(ClipboardEntry {
            id: self.id,
            content_type: ClipboardContentType::from_db_str(&self.content_type),
            text_content: self.text_content,
            image_path: self.image_path,
            thumbnail_path: self.thumbnail_path,
            file_paths,
            pinned: self.pinned != 0,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

fn db_err(e: rusqlite::Error) -> ShelfError {
    ShelfError::Database(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db::Db;
    use std::path::PathBuf;

    fn setup() -> (Db, tempfile_cache::TempCacheDir) {
        let db = Db::connect(Path::new(":memory:")).unwrap();
        let cache_dir = tempfile_cache::TempCacheDir::new();
        (db, cache_dir)
    }

    /// テスト用の一時ディレクトリ（外部crateを増やさず、std::env::temp_dir + プロセスIDで代用する簡易実装）。
    mod tempfile_cache {
        use std::path::PathBuf;

        pub struct TempCacheDir(pub PathBuf);

        impl TempCacheDir {
            pub fn new() -> Self {
                let dir = std::env::temp_dir().join(format!(
                    "shelf-drop-test-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ));
                Self(dir)
            }
        }

        impl Drop for TempCacheDir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn record_and_list_text_entry() {
        let (db, cache) = setup();
        let conn = db.0.lock().unwrap();

        let snapshot = ClipboardSnapshot::Text("hello".to_string());
        let hash = content_hash(&snapshot);
        record_entry(&conn, &cache.0, &snapshot, &hash).unwrap();

        let entries = list_history(&conn, None).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].text_content.as_deref(), Some("hello"));
        assert!(!entries[0].pinned);
    }

    #[test]
    fn duplicate_content_updates_timestamp_instead_of_inserting() {
        let (db, cache) = setup();
        let conn = db.0.lock().unwrap();

        let snapshot = ClipboardSnapshot::Text("dup".to_string());
        let hash = content_hash(&snapshot);
        record_entry(&conn, &cache.0, &snapshot, &hash).unwrap();
        record_entry(&conn, &cache.0, &snapshot, &hash).unwrap();

        let entries = list_history(&conn, None).unwrap();
        assert_eq!(entries.len(), 1, "同一内容は新規行を作らず1件のままのはず");
    }

    #[test]
    fn pinned_state_is_preserved_across_duplicate_recording() {
        let (db, cache) = setup();
        let conn = db.0.lock().unwrap();

        let snapshot = ClipboardSnapshot::Text("keep-pin".to_string());
        let hash = content_hash(&snapshot);
        record_entry(&conn, &cache.0, &snapshot, &hash).unwrap();
        let id = list_history(&conn, None).unwrap()[0].id;
        set_pinned(&conn, id, true).unwrap();

        record_entry(&conn, &cache.0, &snapshot, &hash).unwrap();

        let entries = list_history(&conn, None).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].pinned, "重複記録後もピン留め状態が保持されるはず");
    }

    #[test]
    fn image_snapshot_is_saved_to_cache_dir() {
        let (db, cache) = setup();
        let conn = db.0.lock().unwrap();

        let bytes = vec![1u8, 2, 3, 4];
        let snapshot = ClipboardSnapshot::Image(bytes);
        let hash = content_hash(&snapshot);
        record_entry(&conn, &cache.0, &snapshot, &hash).unwrap();

        let entries = list_history(&conn, None).unwrap();
        let image_path = entries[0].image_path.clone().unwrap();
        assert!(PathBuf::from(&image_path).exists());
    }

    #[test]
    fn delete_entry_removes_row_and_image_file() {
        let (db, cache) = setup();
        let conn = db.0.lock().unwrap();

        let snapshot = ClipboardSnapshot::Image(vec![9, 9, 9]);
        let hash = content_hash(&snapshot);
        record_entry(&conn, &cache.0, &snapshot, &hash).unwrap();
        let entries = list_history(&conn, None).unwrap();
        let image_path = entries[0].image_path.clone().unwrap();

        delete_entry(&conn, entries[0].id).unwrap();

        assert!(list_history(&conn, None).unwrap().is_empty());
        assert!(!PathBuf::from(&image_path).exists());
    }

    #[test]
    fn clear_excludes_pinned_when_requested() {
        let (db, cache) = setup();
        let conn = db.0.lock().unwrap();

        record_entry(&conn, &cache.0, &ClipboardSnapshot::Text("a".into()), &content_hash(&ClipboardSnapshot::Text("a".into()))).unwrap();
        record_entry(&conn, &cache.0, &ClipboardSnapshot::Text("b".into()), &content_hash(&ClipboardSnapshot::Text("b".into()))).unwrap();
        let entries = list_history(&conn, None).unwrap();
        let pinned_id = entries.iter().find(|e| e.text_content.as_deref() == Some("a")).unwrap().id;
        set_pinned(&conn, pinned_id, true).unwrap();

        clear(&conn, true).unwrap();

        let remaining = list_history(&conn, None).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].text_content.as_deref(), Some("a"));
    }

    #[test]
    fn enforce_retention_by_count_deletes_oldest_unpinned_first() {
        let (db, cache) = setup();
        let conn = db.0.lock().unwrap();

        for i in 0..5 {
            let text = format!("item-{i}");
            let snapshot = ClipboardSnapshot::Text(text);
            let hash = content_hash(&snapshot);
            record_entry(&conn, &cache.0, &snapshot, &hash).unwrap();
        }

        enforce_retention(&conn, 3, 30).unwrap();

        let remaining = list_history(&conn, None).unwrap();
        assert_eq!(remaining.len(), 3, "件数上限を超えた分は削除されるはず");
    }

    #[test]
    fn list_history_filters_by_query() {
        let (db, cache) = setup();
        let conn = db.0.lock().unwrap();

        record_entry(&conn, &cache.0, &ClipboardSnapshot::Text("apple pie".into()), &content_hash(&ClipboardSnapshot::Text("apple pie".into()))).unwrap();
        record_entry(&conn, &cache.0, &ClipboardSnapshot::Text("banana".into()), &content_hash(&ClipboardSnapshot::Text("banana".into()))).unwrap();

        let results = list_history(&conn, Some("apple")).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text_content.as_deref(), Some("apple pie"));
    }
}
