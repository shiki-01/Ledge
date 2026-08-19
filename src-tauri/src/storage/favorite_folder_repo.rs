//! `favorite_folders`テーブルへのCRUD操作（Phase6, F-09）。SQL文はこのファイルに閉じ込める。

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::ShelfError;

use super::models::FavoriteFolder;

/// よく使うフォルダの一覧を取得する。
/// 並び順は`sort_order, id`昇順（登録順、architecture.md 12.1章）。
pub fn list_items(conn: &Connection) -> Result<Vec<FavoriteFolder>, ShelfError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, folder_path, display_name, sort_order, added_at \
             FROM favorite_folders ORDER BY sort_order ASC, id ASC",
        )
        .map_err(db_err)?;

    let rows = stmt
        .query_map([], |row| {
            let folder_path: String = row.get(1)?;
            Ok(FavoriteFolder {
                id: row.get(0)?,
                missing: !Path::new(&folder_path).is_dir(),
                folder_path,
                display_name: row.get(2)?,
                sort_order: row.get(3)?,
                added_at: row.get(4)?,
            })
        })
        .map_err(db_err)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

/// フォルダをよく使うフォルダとして登録する。
///
/// `path`がフォルダでない場合はエラーとする。同一フォルダの重複登録は
/// `folder_path`のUNIQUE制約違反として`ShelfError::Conflict`にマッピングする。
pub fn add(conn: &Connection, path: &str) -> Result<FavoriteFolder, ShelfError> {
    let folder_path = Path::new(path);
    if !folder_path.is_dir() {
        return Err(ShelfError::Internal("指定されたパスはフォルダではありません".into()));
    }

    let display_name = folder_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());

    let next_sort_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM favorite_folders",
            [],
            |row| row.get(0),
        )
        .map_err(db_err)?;

    conn.execute(
        "INSERT INTO favorite_folders (folder_path, display_name, sort_order) VALUES (?1, ?2, ?3)",
        params![path, display_name, next_sort_order],
    )
    .map_err(|e| {
        if is_unique_violation(&e) {
            ShelfError::Conflict("このフォルダは既に登録されています".into())
        } else {
            db_err(e)
        }
    })?;

    let id = conn.last_insert_rowid();
    let added_at: String = conn
        .query_row(
            "SELECT added_at FROM favorite_folders WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(db_err)?;

    Ok(FavoriteFolder {
        id,
        folder_path: path.to_string(),
        display_name,
        sort_order: next_sort_order,
        added_at,
        missing: !folder_path.is_dir(),
    })
}

/// 個別削除。
pub fn remove(conn: &Connection, id: i64) -> Result<(), ShelfError> {
    let affected = conn
        .execute("DELETE FROM favorite_folders WHERE id = ?1", params![id])
        .map_err(db_err)?;
    if affected == 0 {
        return Err(ShelfError::NotFound(format!("favorite folder id={id}")));
    }
    Ok(())
}

/// 指定idのフォルダパスを取得する（F-03のドラッグ開始用）。
pub fn get_path(conn: &Connection, id: i64) -> Result<String, ShelfError> {
    conn.query_row(
        "SELECT folder_path FROM favorite_folders WHERE id = ?1",
        params![id],
        |row| row.get(0),
    )
    .optional()
    .map_err(db_err)?
    .ok_or_else(|| ShelfError::NotFound(format!("favorite folder id={id}")))
}

fn is_unique_violation(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(err, _) if err.code == rusqlite::ErrorCode::ConstraintViolation
    )
}

fn db_err(e: rusqlite::Error) -> ShelfError {
    ShelfError::Database(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db::Db;
    use std::fs;

    fn setup() -> Db {
        Db::connect(Path::new(":memory:")).unwrap()
    }

    /// テスト用に実在する一時ディレクトリを作る（`is_dir()`検証を通すため）。
    fn make_temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ledge-favorite-folder-repo-test-{name}-{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn add_and_list_items() {
        let db = setup();
        let conn = db.0.lock().unwrap();
        let dir = make_temp_dir("add_and_list");
        let path = dir.to_string_lossy().to_string();

        let added = add(&conn, &path).unwrap();
        assert_eq!(added.display_name, dir.file_name().unwrap().to_string_lossy());
        assert!(!added.missing, "実在するフォルダなのでmissingにならないはず");

        let items = list_items(&conn).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].folder_path, path);
    }

    #[test]
    fn add_with_non_directory_path_returns_internal_error() {
        let db = setup();
        let conn = db.0.lock().unwrap();

        let result = add(&conn, "/path/does/not/exist-at-all-12345");
        assert!(matches!(result, Err(ShelfError::Internal(_))));
    }

    #[test]
    fn add_with_duplicate_path_returns_conflict() {
        let db = setup();
        let conn = db.0.lock().unwrap();
        let dir = make_temp_dir("duplicate");
        let path = dir.to_string_lossy().to_string();

        add(&conn, &path).unwrap();
        let result = add(&conn, &path);
        assert!(matches!(result, Err(ShelfError::Conflict(_))));
    }

    #[test]
    fn missing_flag_is_true_when_directory_removed() {
        let db = setup();
        let conn = db.0.lock().unwrap();
        let dir = make_temp_dir("missing_flag");
        let path = dir.to_string_lossy().to_string();

        add(&conn, &path).unwrap();
        fs::remove_dir_all(&dir).unwrap();

        let items = list_items(&conn).unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].missing);
    }

    #[test]
    fn remove_deletes_item() {
        let db = setup();
        let conn = db.0.lock().unwrap();
        let dir = make_temp_dir("remove");
        let path = dir.to_string_lossy().to_string();

        let added = add(&conn, &path).unwrap();
        remove(&conn, added.id).unwrap();

        let items = list_items(&conn).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn remove_errors_when_not_found() {
        let db = setup();
        let conn = db.0.lock().unwrap();

        let result = remove(&conn, 999);
        assert!(matches!(result, Err(ShelfError::NotFound(_))));
    }

    #[test]
    fn get_path_returns_folder_path() {
        let db = setup();
        let conn = db.0.lock().unwrap();
        let dir = make_temp_dir("get_path");
        let path = dir.to_string_lossy().to_string();

        let added = add(&conn, &path).unwrap();
        let fetched = get_path(&conn, added.id).unwrap();
        assert_eq!(fetched, path);
    }

    #[test]
    fn get_path_errors_when_not_found() {
        let db = setup();
        let conn = db.0.lock().unwrap();

        let result = get_path(&conn, 999);
        assert!(matches!(result, Err(ShelfError::NotFound(_))));
    }

    #[test]
    fn sort_order_increments_with_each_add() {
        let db = setup();
        let conn = db.0.lock().unwrap();
        let dir_a = make_temp_dir("sort_a");
        let dir_b = make_temp_dir("sort_b");

        let a = add(&conn, &dir_a.to_string_lossy()).unwrap();
        let b = add(&conn, &dir_b.to_string_lossy()).unwrap();

        assert_eq!(a.sort_order, 0);
        assert_eq!(b.sort_order, 1);
    }
}
