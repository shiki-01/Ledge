//! `tags` / `clipboard_tags`テーブルへのCRUD操作（Phase4, F-17）。SQL文はこのファイルに閉じ込める。

use rusqlite::params;
use rusqlite::Connection;

use crate::error::ShelfError;

use super::models::Tag;

/// タグ一覧を取得する（名前昇順）。
pub fn list_tags(conn: &Connection) -> Result<Vec<Tag>, ShelfError> {
    let mut stmt = conn
        .prepare("SELECT id, name, color FROM tags ORDER BY name ASC")
        .map_err(db_err)?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Tag {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
            })
        })
        .map_err(db_err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

/// タグを作成する。`name`は`UNIQUE`制約があるため、重複時は`ShelfError::Conflict`を返す
/// （architecture.md 9.3章）。
pub fn create_tag(conn: &Connection, name: &str, color: Option<&str>) -> Result<Tag, ShelfError> {
    conn.execute(
        "INSERT INTO tags (name, color) VALUES (?1, ?2)",
        params![name, color],
    )
    .map_err(|e| {
        if is_unique_violation(&e) {
            ShelfError::Conflict(format!("タグ名「{name}」は既に使用されています"))
        } else {
            db_err(e)
        }
    })?;

    Ok(Tag {
        id: conn.last_insert_rowid(),
        name: name.to_string(),
        color: color.map(|c| c.to_string()),
    })
}

/// タグを削除する。`clipboard_tags`は`ON DELETE CASCADE`のため関連付けも自動削除される。
pub fn delete_tag(conn: &Connection, id: i64) -> Result<(), ShelfError> {
    let affected = conn
        .execute("DELETE FROM tags WHERE id = ?1", params![id])
        .map_err(db_err)?;
    if affected == 0 {
        return Err(ShelfError::NotFound(format!("tag id={id}")));
    }
    Ok(())
}

/// 指定エントリのタグ付けを一括置換する（差分diffではなく全置換、architecture.md 9.3章:
/// 「差分diffではなく全置換が実装しやすくバグりにくいための判断」）。
pub fn set_clipboard_tags(
    conn: &Connection,
    clipboard_id: i64,
    tag_ids: &[i64],
) -> Result<(), ShelfError> {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM clipboard_history WHERE id = ?1)",
            params![clipboard_id],
            |row| row.get(0),
        )
        .map_err(db_err)?;
    if !exists {
        return Err(ShelfError::NotFound(format!(
            "clipboard entry id={clipboard_id}"
        )));
    }

    conn.execute(
        "DELETE FROM clipboard_tags WHERE clipboard_id = ?1",
        params![clipboard_id],
    )
    .map_err(db_err)?;
    for tag_id in tag_ids {
        conn.execute(
            "INSERT INTO clipboard_tags (clipboard_id, tag_id) VALUES (?1, ?2)",
            params![clipboard_id, tag_id],
        )
        .map_err(db_err)?;
    }
    Ok(())
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
    use std::path::Path;

    fn setup() -> Db {
        Db::connect(Path::new(":memory:")).unwrap()
    }

    #[test]
    fn create_and_list_tags() {
        let db = setup();
        let conn = db.0.lock().unwrap();

        create_tag(&conn, "work", Some("#3b82f6")).unwrap();
        create_tag(&conn, "personal", None).unwrap();

        let tags = list_tags(&conn).unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "personal", "name昇順で並ぶはず");
    }

    #[test]
    fn create_tag_with_duplicate_name_returns_conflict() {
        let db = setup();
        let conn = db.0.lock().unwrap();

        create_tag(&conn, "work", None).unwrap();
        let result = create_tag(&conn, "work", None);
        assert!(matches!(result, Err(ShelfError::Conflict(_))));
    }

    #[test]
    fn delete_tag_errors_when_not_found() {
        let db = setup();
        let conn = db.0.lock().unwrap();

        let result = delete_tag(&conn, 999);
        assert!(matches!(result, Err(ShelfError::NotFound(_))));
    }

    #[test]
    fn set_clipboard_tags_replaces_existing_associations() {
        let db = setup();
        let conn = db.0.lock().unwrap();

        conn.execute(
            "INSERT INTO clipboard_history (content_type, text_content, content_hash) VALUES ('text', 'hello', 'hash1')",
            [],
        )
        .unwrap();
        let clipboard_id = conn.last_insert_rowid();

        let tag_a = create_tag(&conn, "a", None).unwrap();
        let tag_b = create_tag(&conn, "b", None).unwrap();

        set_clipboard_tags(&conn, clipboard_id, &[tag_a.id, tag_b.id]).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_tags WHERE clipboard_id = ?1",
                params![clipboard_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);

        // 全置換: aだけにする
        set_clipboard_tags(&conn, clipboard_id, &[tag_a.id]).unwrap();
        let remaining: Vec<i64> = {
            let mut stmt = conn
                .prepare("SELECT tag_id FROM clipboard_tags WHERE clipboard_id = ?1")
                .unwrap();
            stmt.query_map(params![clipboard_id], |row| row.get(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(remaining, vec![tag_a.id]);
    }

    #[test]
    fn set_clipboard_tags_errors_when_entry_not_found() {
        let db = setup();
        let conn = db.0.lock().unwrap();

        let result = set_clipboard_tags(&conn, 999, &[]);
        assert!(matches!(result, Err(ShelfError::NotFound(_))));
    }
}
