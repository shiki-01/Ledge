//! `shelf_items`テーブルへのCRUD操作。SQL文はこのファイルに閉じ込める。

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::ShelfError;

use super::models::{ShelfItem, ShelfItemType};

/// シェルフ内アイテムの一覧を取得する。
/// 並び順は「追加日時降順固定」（requirements.md 10.1章、手動並び替えは将来検討）。
pub fn list_items(conn: &Connection) -> Result<Vec<ShelfItem>, ShelfError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, item_type, source_path, display_name, size_bytes, locked, sort_order, added_at \
             FROM shelf_items ORDER BY added_at DESC, id DESC",
        )
        .map_err(db_err)?;

    let rows = stmt
        .query_map([], |row| {
            let item_type: String = row.get(1)?;
            let source_path: String = row.get(2)?;
            let locked: i64 = row.get(5)?;
            let modified_at_ms = read_modified_at_ms(Path::new(&source_path));
            Ok(ShelfItem {
                id: row.get(0)?,
                item_type: ShelfItemType::from_db_str(&item_type),
                missing: !Path::new(&source_path).exists(),
                source_path,
                display_name: row.get(3)?,
                size_bytes: row.get(4)?,
                locked: locked != 0,
                sort_order: row.get(6)?,
                added_at: row.get(7)?,
                modified_at_ms,
            })
        })
        .map_err(db_err)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

/// パス群をシェルフへ追加する。
///
/// 同一パスの再ドロップも許可し、それぞれ別アイテムとして追加する
/// （requirements.md 10.1章: 重複ドロップ方針）。フォルダは中身を展開せず、
/// フォルダパス自体を1アイテムとして保持する。
pub fn add_paths(conn: &Connection, paths: &[String]) -> Result<Vec<ShelfItem>, ShelfError> {
    let next_sort_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM shelf_items",
            [],
            |row| row.get(0),
        )
        .map_err(db_err)?;

    let mut added = Vec::with_capacity(paths.len());

    for (offset, path_str) in paths.iter().enumerate() {
        let path = Path::new(path_str);
        let item_type = if path.is_dir() {
            ShelfItemType::Folder
        } else {
            ShelfItemType::File
        };
        let display_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path_str.clone());
        // フォルダはサイズ集計を行わない（実体コピーをしない方針のため、都度の再帰走査コストも避ける）
        let size_bytes: Option<i64> = if item_type == ShelfItemType::File {
            std::fs::metadata(path).ok().map(|m| m.len() as i64)
        } else {
            None
        };
        let sort_order = next_sort_order + offset as i64;

        conn.execute(
            "INSERT INTO shelf_items (item_type, source_path, display_name, size_bytes, locked, sort_order) \
             VALUES (?1, ?2, ?3, ?4, 0, ?5)",
            params![
                item_type.as_db_str(),
                path_str,
                display_name,
                size_bytes,
                sort_order
            ],
        )
        .map_err(db_err)?;

        let id = conn.last_insert_rowid();
        let added_at: String = conn
            .query_row(
                "SELECT added_at FROM shelf_items WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(db_err)?;

        added.push(ShelfItem {
            id,
            item_type,
            source_path: path_str.clone(),
            display_name,
            size_bytes,
            locked: false,
            sort_order,
            added_at,
            missing: !path.exists(),
            modified_at_ms: read_modified_at_ms(path),
        });
    }

    Ok(added)
}

/// ファイルの最終更新日時をUnixエポックミリ秒で取得する（F-07プレビュー用）。
/// 存在しない/取得できない場合は`None`を返す（`missing`判定と同様、エラーにはしない）。
fn read_modified_at_ms(path: &Path) -> Option<i64> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    let duration = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    i64::try_from(duration.as_millis()).ok()
}

/// 個別削除。
pub fn remove_item(conn: &Connection, id: i64) -> Result<(), ShelfError> {
    let affected = conn
        .execute("DELETE FROM shelf_items WHERE id = ?1", params![id])
        .map_err(db_err)?;
    if affected == 0 {
        return Err(ShelfError::NotFound(format!("shelf item id={id}")));
    }
    Ok(())
}

/// ロック状態を変更する（F-06）。
pub fn set_locked(conn: &Connection, id: i64, locked: bool) -> Result<(), ShelfError> {
    let affected = conn
        .execute(
            "UPDATE shelf_items SET locked = ?1 WHERE id = ?2",
            params![locked as i64, id],
        )
        .map_err(db_err)?;
    if affected == 0 {
        return Err(ShelfError::NotFound(format!("shelf item id={id}")));
    }
    Ok(())
}

/// 一括削除。
///
/// `exclude_locked=true`の場合、ロック済みアイテム（F-06）は削除対象から除外する
/// （requirements.md 10.1章）。個別削除（`remove_item`）はロック状態にかかわらず可能。
pub fn clear(conn: &Connection, exclude_locked: bool) -> Result<(), ShelfError> {
    if exclude_locked {
        conn.execute("DELETE FROM shelf_items WHERE locked = 0", [])
    } else {
        conn.execute("DELETE FROM shelf_items", [])
    }
    .map_err(db_err)?;
    Ok(())
}

/// 指定id群のsource_pathを取得する（F-03のドラッグ開始用）。
/// 存在しないidは黙って無視する。
pub fn get_paths(conn: &Connection, ids: &[i64]) -> Result<Vec<String>, ShelfError> {
    let mut paths = Vec::with_capacity(ids.len());
    for id in ids {
        let path: Option<String> = conn
            .query_row(
                "SELECT source_path FROM shelf_items WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_err)?;
        if let Some(path) = path {
            paths.push(path);
        }
    }
    Ok(paths)
}

fn db_err(e: rusqlite::Error) -> ShelfError {
    ShelfError::Database(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db::Db;

    fn setup() -> Db {
        Db::connect(Path::new(":memory:")).unwrap()
    }

    #[test]
    fn add_and_list_items() {
        let db = setup();
        let conn = db.0.lock().unwrap();

        let added = add_paths(&conn, &["/tmp/example.txt".to_string()]).unwrap();
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].display_name, "example.txt");
        assert!(added[0].missing, "存在しないパスなのでmissingになるはず");

        let items = list_items(&conn).unwrap();
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn duplicate_paths_are_added_as_separate_items() {
        let db = setup();
        let conn = db.0.lock().unwrap();

        add_paths(
            &conn,
            &[
                "/tmp/dup.txt".to_string(),
                "/tmp/dup.txt".to_string(),
            ],
        )
        .unwrap();

        let items = list_items(&conn).unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn remove_item_errors_when_not_found() {
        let db = setup();
        let conn = db.0.lock().unwrap();

        let result = remove_item(&conn, 999);
        assert!(matches!(result, Err(ShelfError::NotFound(_))));
    }

    #[test]
    fn clear_with_exclude_locked_false_removes_all_items() {
        let db = setup();
        let conn = db.0.lock().unwrap();

        add_paths(&conn, &["/tmp/a.txt".to_string(), "/tmp/b.txt".to_string()]).unwrap();
        clear(&conn, false).unwrap();

        let items = list_items(&conn).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn clear_with_exclude_locked_true_keeps_locked_items() {
        let db = setup();
        let conn = db.0.lock().unwrap();

        let added = add_paths(&conn, &["/tmp/a.txt".to_string(), "/tmp/b.txt".to_string()]).unwrap();
        set_locked(&conn, added[0].id, true).unwrap();

        clear(&conn, true).unwrap();

        let items = list_items(&conn).unwrap();
        assert_eq!(items.len(), 1, "ロック済みアイテムだけが残るはず");
        assert_eq!(items[0].id, added[0].id);
        assert!(items[0].locked);
    }

    #[test]
    fn set_locked_toggles_flag() {
        let db = setup();
        let conn = db.0.lock().unwrap();

        let added = add_paths(&conn, &["/tmp/a.txt".to_string()]).unwrap();
        assert!(!added[0].locked);

        set_locked(&conn, added[0].id, true).unwrap();
        let items = list_items(&conn).unwrap();
        assert!(items[0].locked);

        set_locked(&conn, added[0].id, false).unwrap();
        let items = list_items(&conn).unwrap();
        assert!(!items[0].locked);
    }

    #[test]
    fn set_locked_errors_when_not_found() {
        let db = setup();
        let conn = db.0.lock().unwrap();

        let result = set_locked(&conn, 999, true);
        assert!(matches!(result, Err(ShelfError::NotFound(_))));
    }
}
