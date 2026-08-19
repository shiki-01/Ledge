//! `clipboard_history`テーブルへのCRUD操作（Phase2, F-11〜F-13, F-16）。SQL文はこのファイルに閉じ込める。
//!
//! 画像はDBにBLOB格納せず、呼び出し元から渡されるキャッシュディレクトリ配下にPNGファイルとして
//! 保存し、パスのみDBに記録する（requirements.md 10.2章）。

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::clipboard::ClipboardSnapshot;
use crate::error::ShelfError;

use super::models::{ClipboardContentType, ClipboardEntry, Tag};

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

/// LIKE検索のワイルドカード文字（`%` `_`）とエスケープ文字自体（`\`）をエスケープする（F-14）。
/// `LIKE ?1 ESCAPE '\'`と組み合わせることで、ユーザー入力中の`%`/`_`をリテラル文字として扱う
/// （既知の課題の修正、architecture.md 9.1章）。
fn escape_like_pattern(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        if matches!(c, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

/// 履歴一覧を取得する。`query`が指定されている場合はテキスト内容/ファイルパス/タグ名に対する
/// LIKE検索を行う（F-14, architecture.md 9.1章）。`tag_id`が指定されている場合はそのタグが
/// 付与されたエントリのみに絞り込む（F-17）。
/// ピン留めアイテムを先頭にまとめ、その中・それ以外それぞれ更新日時降順で並べる。
pub fn list_history(
    conn: &Connection,
    query: Option<&str>,
    tag_id: Option<i64>,
) -> Result<Vec<ClipboardEntry>, ShelfError> {
    let like_param = query.map(|q| format!("%{}%", escape_like_pattern(q)));

    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT ch.id, ch.content_type, ch.text_content, ch.image_path, ch.thumbnail_path, \
                    ch.file_paths_json, ch.pinned, ch.created_at, ch.updated_at, ch.content_hash \
             FROM clipboard_history ch \
             LEFT JOIN clipboard_tags ct ON ct.clipboard_id = ch.id \
             LEFT JOIN tags t ON t.id = ct.tag_id \
             WHERE (?1 IS NULL OR ch.text_content LIKE ?1 ESCAPE '\\' \
                    OR ch.file_paths_json LIKE ?1 ESCAPE '\\' OR t.name LIKE ?1 ESCAPE '\\') \
               AND (?2 IS NULL OR ct.tag_id = ?2) \
             ORDER BY ch.pinned DESC, ch.updated_at DESC, ch.id DESC",
        )
        .map_err(db_err)?;

    let rows = stmt
        .query_map(params![like_param, tag_id], |row| {
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
                content_hash: row.get(9)?,
            })
        })
        .map_err(db_err)?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.map_err(db_err)?.into_entry()?);
    }
    attach_tags(conn, entries)
}

/// idを指定して1件取得する（F-12: 履歴からの貼り付け用）。
pub fn get_entry(conn: &Connection, id: i64) -> Result<ClipboardEntry, ShelfError> {
    let row = conn
        .query_row(
            "SELECT id, content_type, text_content, image_path, thumbnail_path, file_paths_json, \
                    pinned, created_at, updated_at, content_hash \
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
                    content_hash: row.get(9)?,
                })
            },
        )
        .optional()
        .map_err(db_err)?;

    let entry = match row {
        Some(row) => row.into_entry()?,
        None => return Err(ShelfError::NotFound(format!("clipboard entry id={id}"))),
    };

    attach_tags(conn, vec![entry])?
        .into_iter()
        .next()
        .ok_or_else(|| ShelfError::NotFound(format!("clipboard entry id={id}")))
}

/// 複数のテキストエントリを改行結合し、新規テキストエントリとして記録する（F-15）。
/// 対象は`content_type = 'text'`のみ。`ids`の並び順（一覧の表示順に対応、architecture.md 9.2章）
/// で結合する。元アイテムは削除しない（結合はコピーであり移動ではない、という裁量判断）。
pub fn stack_entries(conn: &Connection, cache_dir: &Path, ids: &[i64]) -> Result<ClipboardEntry, ShelfError> {
    if ids.len() < 2 {
        return Err(ShelfError::Internal("スタックには2件以上のテキストアイテムを選択してください".into()));
    }

    let mut texts = Vec::with_capacity(ids.len());
    for id in ids {
        let row: Option<(String, Option<String>)> = conn
            .query_row(
                "SELECT content_type, text_content FROM clipboard_history WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(db_err)?;

        let (content_type, text_content) =
            row.ok_or_else(|| ShelfError::NotFound(format!("clipboard entry id={id}")))?;

        if content_type != ClipboardContentType::Text.as_db_str() {
            return Err(ShelfError::Internal(
                "テキスト以外のアイテムはスタック（結合）できません".into(),
            ));
        }
        texts.push(text_content.unwrap_or_default());
    }

    let snapshot = ClipboardSnapshot::Text(texts.join("\n"));
    let hash = content_hash(&snapshot);
    record_entry(conn, cache_dir, &snapshot, &hash)?;

    let new_id: i64 = conn
        .query_row(
            "SELECT id FROM clipboard_history WHERE content_hash = ?1",
            params![hash],
            |row| row.get(0),
        )
        .map_err(db_err)?;

    get_entry(conn, new_id)
}

/// 一覧取得結果にタグを付与する（1クエリで対象id全件分をまとめて取得する）。
fn attach_tags(conn: &Connection, mut entries: Vec<ClipboardEntry>) -> Result<Vec<ClipboardEntry>, ShelfError> {
    if entries.is_empty() {
        return Ok(entries);
    }

    let ids: Vec<i64> = entries.iter().map(|e| e.id).collect();
    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!(
        "SELECT ct.clipboard_id, t.id, t.name, t.color FROM clipboard_tags ct \
         JOIN tags t ON t.id = ct.tag_id WHERE ct.clipboard_id IN ({placeholders}) ORDER BY t.name ASC"
    );

    let mut stmt = conn.prepare(&sql).map_err(db_err)?;
    let rows = stmt
        .query_map(params_from_iter(ids.iter()), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                Tag {
                    id: row.get(1)?,
                    name: row.get(2)?,
                    color: row.get(3)?,
                },
            ))
        })
        .map_err(db_err)?;

    let mut tags_by_clipboard_id: HashMap<i64, Vec<Tag>> = HashMap::new();
    for row in rows {
        let (clipboard_id, tag) = row.map_err(db_err)?;
        tags_by_clipboard_id.entry(clipboard_id).or_default().push(tag);
    }

    for entry in &mut entries {
        entry.tags = tags_by_clipboard_id.remove(&entry.id).unwrap_or_default();
    }

    Ok(entries)
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

/// F-22（デバイス間同期）: クラウド（Firestore）側の変更をローカルへ反映する（pull）。
///
/// `content_hash`一致の既存行があれば`updated_at`を比較し、クラウド側が新しい場合のみ
/// `text_content` / `pinned = 1` / `updated_at`を更新する（Last-Write-Wins、architecture.md
/// 10.2章）。`updated_at`はRust側の`strftime('%Y-%m-%dT%H:%M:%fZ', 'now')`と同じISO 8601
/// （ミリ秒3桁・UTC）形式である前提で、SQLite上は単純な文字列比較で新旧を判定できる。
/// 既存行が無ければ`content_type = 'text'`のピン留め済み新規行として挿入する。
///
/// 常にpinned扱いで挿入し、タイムスタンプをクラウド側の値で上書きする点が`record_entry`とは
/// 異なるため、`record_entry`は流用せず専用関数として実装している（呼び出し元指示のとおり）。
pub fn sync_upsert_from_cloud(
    conn: &Connection,
    content_hash: &str,
    text_content: &str,
    updated_at: &str,
) -> Result<(), ShelfError> {
    let existing: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, updated_at FROM clipboard_history WHERE content_hash = ?1",
            params![content_hash],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(db_err)?;

    match existing {
        Some((id, local_updated_at)) => {
            // クラウド側が新しい場合のみ上書きする（Last-Write-Wins）。同値・ローカルの方が新しい
            // 場合は何もしない（ローカルの方が新しいのにクラウドの古い値で巻き戻さないため）。
            if updated_at > local_updated_at.as_str() {
                conn.execute(
                    "UPDATE clipboard_history SET text_content = ?1, pinned = 1, updated_at = ?2 WHERE id = ?3",
                    params![text_content, updated_at, id],
                )
                .map_err(db_err)?;
            }
        }
        None => {
            conn.execute(
                "INSERT INTO clipboard_history \
                    (content_type, text_content, pinned, content_hash, created_at, updated_at) \
                 VALUES ('text', ?1, 1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?3)",
                params![text_content, content_hash, updated_at],
            )
            .map_err(db_err)?;
        }
    }

    Ok(())
}

/// F-22（デバイス間同期）: クラウド側でドキュメントが削除されたことをローカルへ反映する（pull）。
///
/// 該当行があれば`pinned = 0`に更新するのみで、行自体は削除しない（安全側の設計判断、
/// architecture.md 10.2章）。該当行が無ければ何もしない（エラーにしない）。
pub fn sync_unpin_by_hash(conn: &Connection, content_hash: &str) -> Result<(), ShelfError> {
    conn.execute(
        "UPDATE clipboard_history SET pinned = 0 WHERE content_hash = ?1",
        params![content_hash],
    )
    .map_err(db_err)?;
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
    content_hash: String,
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
            content_hash: self.content_hash,
            tags: Vec::new(),
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
                    "ledge-test-{}-{}",
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

        let entries = list_history(&conn, None, None).unwrap();
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

        let entries = list_history(&conn, None, None).unwrap();
        assert_eq!(entries.len(), 1, "同一内容は新規行を作らず1件のままのはず");
    }

    #[test]
    fn pinned_state_is_preserved_across_duplicate_recording() {
        let (db, cache) = setup();
        let conn = db.0.lock().unwrap();

        let snapshot = ClipboardSnapshot::Text("keep-pin".to_string());
        let hash = content_hash(&snapshot);
        record_entry(&conn, &cache.0, &snapshot, &hash).unwrap();
        let id = list_history(&conn, None, None).unwrap()[0].id;
        set_pinned(&conn, id, true).unwrap();

        record_entry(&conn, &cache.0, &snapshot, &hash).unwrap();

        let entries = list_history(&conn, None, None).unwrap();
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

        let entries = list_history(&conn, None, None).unwrap();
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
        let entries = list_history(&conn, None, None).unwrap();
        let image_path = entries[0].image_path.clone().unwrap();

        delete_entry(&conn, entries[0].id).unwrap();

        assert!(list_history(&conn, None, None).unwrap().is_empty());
        assert!(!PathBuf::from(&image_path).exists());
    }

    #[test]
    fn clear_excludes_pinned_when_requested() {
        let (db, cache) = setup();
        let conn = db.0.lock().unwrap();

        record_entry(&conn, &cache.0, &ClipboardSnapshot::Text("a".into()), &content_hash(&ClipboardSnapshot::Text("a".into()))).unwrap();
        record_entry(&conn, &cache.0, &ClipboardSnapshot::Text("b".into()), &content_hash(&ClipboardSnapshot::Text("b".into()))).unwrap();
        let entries = list_history(&conn, None, None).unwrap();
        let pinned_id = entries.iter().find(|e| e.text_content.as_deref() == Some("a")).unwrap().id;
        set_pinned(&conn, pinned_id, true).unwrap();

        clear(&conn, true).unwrap();

        let remaining = list_history(&conn, None, None).unwrap();
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

        let remaining = list_history(&conn, None, None).unwrap();
        assert_eq!(remaining.len(), 3, "件数上限を超えた分は削除されるはず");
    }

    #[test]
    fn list_history_filters_by_query() {
        let (db, cache) = setup();
        let conn = db.0.lock().unwrap();

        record_entry(&conn, &cache.0, &ClipboardSnapshot::Text("apple pie".into()), &content_hash(&ClipboardSnapshot::Text("apple pie".into()))).unwrap();
        record_entry(&conn, &cache.0, &ClipboardSnapshot::Text("banana".into()), &content_hash(&ClipboardSnapshot::Text("banana".into()))).unwrap();

        let results = list_history(&conn, Some("apple"), None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text_content.as_deref(), Some("apple pie"));
    }

    #[test]
    fn list_history_query_escapes_like_wildcards() {
        // F-14既知の課題の修正確認: "%"/"_"がワイルドカードとして展開されず、リテラルとして扱われること
        let (db, cache) = setup();
        let conn = db.0.lock().unwrap();

        record_entry(&conn, &cache.0, &ClipboardSnapshot::Text("50% off".into()), &content_hash(&ClipboardSnapshot::Text("50% off".into()))).unwrap();
        record_entry(&conn, &cache.0, &ClipboardSnapshot::Text("50 dollars off".into()), &content_hash(&ClipboardSnapshot::Text("50 dollars off".into()))).unwrap();

        let results = list_history(&conn, Some("50%"), None).unwrap();
        assert_eq!(results.len(), 1, "'%'はワイルドカードではなくリテラル文字として一致するはず");
        assert_eq!(results[0].text_content.as_deref(), Some("50% off"));
    }

    #[test]
    fn list_history_filters_by_tag_id() {
        let (db, cache) = setup();
        let conn = db.0.lock().unwrap();

        record_entry(&conn, &cache.0, &ClipboardSnapshot::Text("tagged".into()), &content_hash(&ClipboardSnapshot::Text("tagged".into()))).unwrap();
        record_entry(&conn, &cache.0, &ClipboardSnapshot::Text("untagged".into()), &content_hash(&ClipboardSnapshot::Text("untagged".into()))).unwrap();
        let entries = list_history(&conn, None, None).unwrap();
        let tagged_id = entries.iter().find(|e| e.text_content.as_deref() == Some("tagged")).unwrap().id;

        let tag = crate::storage::tags_repo::create_tag(&conn, "work", None).unwrap();
        crate::storage::tags_repo::set_clipboard_tags(&conn, tagged_id, &[tag.id]).unwrap();

        let filtered = list_history(&conn, None, Some(tag.id)).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, tagged_id);
        assert_eq!(filtered[0].tags.len(), 1);
        assert_eq!(filtered[0].tags[0].name, "work");
    }

    #[test]
    fn stack_entries_combines_text_items_in_order_without_deleting_originals() {
        let (db, cache) = setup();
        let conn = db.0.lock().unwrap();

        record_entry(&conn, &cache.0, &ClipboardSnapshot::Text("first".into()), &content_hash(&ClipboardSnapshot::Text("first".into()))).unwrap();
        record_entry(&conn, &cache.0, &ClipboardSnapshot::Text("second".into()), &content_hash(&ClipboardSnapshot::Text("second".into()))).unwrap();
        let entries = list_history(&conn, None, None).unwrap();
        let first_id = entries.iter().find(|e| e.text_content.as_deref() == Some("first")).unwrap().id;
        let second_id = entries.iter().find(|e| e.text_content.as_deref() == Some("second")).unwrap().id;

        let stacked = stack_entries(&conn, &cache.0, &[first_id, second_id]).unwrap();
        assert_eq!(stacked.text_content.as_deref(), Some("first\nsecond"));

        let all = list_history(&conn, None, None).unwrap();
        assert_eq!(all.len(), 3, "元の2件は削除されず、結合結果が新規1件として追加されるはず");
    }

    #[test]
    fn sync_upsert_from_cloud_inserts_new_pinned_text_entry() {
        let (db, _cache) = setup();
        let conn = db.0.lock().unwrap();

        sync_upsert_from_cloud(&conn, "hash-1", "from cloud", "2026-08-19T00:00:00.000Z").unwrap();

        let entries = list_history(&conn, None, None).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].text_content.as_deref(), Some("from cloud"));
        assert!(entries[0].pinned, "クラウドからの新規行は常にピン留め済みで挿入されるはず");
        assert_eq!(entries[0].content_hash, "hash-1");
    }

    #[test]
    fn sync_upsert_from_cloud_overwrites_when_cloud_is_newer() {
        let (db, cache) = setup();
        let conn = db.0.lock().unwrap();

        let snapshot = ClipboardSnapshot::Text("local version".into());
        let hash = content_hash(&snapshot);
        record_entry(&conn, &cache.0, &snapshot, &hash).unwrap();
        let id = list_history(&conn, None, None).unwrap()[0].id;
        // ローカルの更新日時を過去に固定しておき、クラウド側が新しいことを確定させる
        conn.execute(
            "UPDATE clipboard_history SET updated_at = '2020-01-01T00:00:00.000Z' WHERE id = ?1",
            params![id],
        )
        .unwrap();

        sync_upsert_from_cloud(&conn, &hash, "cloud version", "2026-08-19T00:00:00.000Z").unwrap();

        let entries = list_history(&conn, None, None).unwrap();
        assert_eq!(entries.len(), 1, "同一content_hashなので新規行にはならないはず");
        assert_eq!(entries[0].text_content.as_deref(), Some("cloud version"));
        assert!(entries[0].pinned);
        assert_eq!(entries[0].updated_at, "2026-08-19T00:00:00.000Z");
    }

    #[test]
    fn sync_upsert_from_cloud_ignores_older_cloud_update() {
        let (db, cache) = setup();
        let conn = db.0.lock().unwrap();

        let snapshot = ClipboardSnapshot::Text("newer local".into());
        let hash = content_hash(&snapshot);
        record_entry(&conn, &cache.0, &snapshot, &hash).unwrap();
        let id = list_history(&conn, None, None).unwrap()[0].id;
        conn.execute(
            "UPDATE clipboard_history SET updated_at = '2026-08-19T00:00:00.000Z' WHERE id = ?1",
            params![id],
        )
        .unwrap();

        // クラウド側の方が古いタイムスタンプを持つ更新は無視されるはず（Last-Write-Wins）
        sync_upsert_from_cloud(&conn, &hash, "stale cloud version", "2020-01-01T00:00:00.000Z").unwrap();

        let entries = list_history(&conn, None, None).unwrap();
        assert_eq!(entries[0].text_content.as_deref(), Some("newer local"));
        assert_eq!(entries[0].updated_at, "2026-08-19T00:00:00.000Z");
    }

    #[test]
    fn sync_unpin_by_hash_unpins_without_deleting_row() {
        let (db, cache) = setup();
        let conn = db.0.lock().unwrap();

        let snapshot = ClipboardSnapshot::Text("keep me".into());
        let hash = content_hash(&snapshot);
        record_entry(&conn, &cache.0, &snapshot, &hash).unwrap();
        let id = list_history(&conn, None, None).unwrap()[0].id;
        set_pinned(&conn, id, true).unwrap();

        sync_unpin_by_hash(&conn, &hash).unwrap();

        let entries = list_history(&conn, None, None).unwrap();
        assert_eq!(entries.len(), 1, "行自体は削除されず残っているはず（安全側の設計判断）");
        assert!(!entries[0].pinned, "ピン留めは解除されるはず");
    }

    #[test]
    fn sync_unpin_by_hash_is_noop_when_hash_not_found() {
        let (db, _cache) = setup();
        let conn = db.0.lock().unwrap();

        // 該当行が無くてもエラーにならないはず
        sync_unpin_by_hash(&conn, "no-such-hash").unwrap();
    }

    #[test]
    fn stack_entries_rejects_non_text_items() {
        let (db, cache) = setup();
        let conn = db.0.lock().unwrap();

        record_entry(&conn, &cache.0, &ClipboardSnapshot::Text("text".into()), &content_hash(&ClipboardSnapshot::Text("text".into()))).unwrap();
        record_entry(&conn, &cache.0, &ClipboardSnapshot::Image(vec![1, 2, 3]), &content_hash(&ClipboardSnapshot::Image(vec![1, 2, 3]))).unwrap();
        let entries = list_history(&conn, None, None).unwrap();
        let ids: Vec<i64> = entries.iter().map(|e| e.id).collect();

        let result = stack_entries(&conn, &cache.0, &ids);
        assert!(result.is_err(), "画像アイテムを含む場合はエラーになるはず");
    }
}
