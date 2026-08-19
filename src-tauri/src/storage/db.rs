//! SQLiteコネクション管理・マイグレーション実行。

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::error::ShelfError;

/// `0001_init.sql`の内容をビルド時に埋め込む。
const MIGRATION_0001: &str = include_str!("../../migrations/0001_init.sql");

/// `0002_favorite_folders.sql`の内容をビルド時に埋め込む（Phase6, F-09）。
const MIGRATION_0002: &str = include_str!("../../migrations/0002_favorite_folders.sql");

/// アプリ全体で共有するSQLiteコネクション。
/// rusqliteの`Connection`は`Send`だが`Sync`ではないため`Mutex`で包み、
/// Tauriコマンドから`tauri::State`経由で共有する。
pub struct Db(pub Mutex<Connection>);

impl Db {
    /// 指定パスのSQLiteファイルに接続し、未適用のマイグレーションを実行する。
    pub fn connect(db_path: &Path) -> Result<Self, ShelfError> {
        let conn = Connection::open(db_path).map_err(db_err)?;
        conn.pragma_update(None, "foreign_keys", true)
            .map_err(db_err)?;
        run_migrations(&conn)?;
        Ok(Self(Mutex::new(conn)))
    }
}

/// `PRAGMA user_version`を使った簡易マイグレーションランナー。
/// バージョン0→1→2と段階的に適用する（Phase6でfavorite_foldersテーブルを追加する0002を追加）。
fn run_migrations(conn: &Connection) -> Result<(), ShelfError> {
    let user_version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(db_err)?;

    if user_version < 1 {
        conn.execute_batch(MIGRATION_0001).map_err(db_err)?;
        conn.pragma_update(None, "user_version", 1)
            .map_err(db_err)?;
    }

    if user_version < 2 {
        conn.execute_batch(MIGRATION_0002).map_err(db_err)?;
        conn.pragma_update(None, "user_version", 2)
            .map_err(db_err)?;
    }

    Ok(())
}

fn db_err(e: rusqlite::Error) -> ShelfError {
    ShelfError::Database(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_migrations_creates_shelf_items_table() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'shelf_items'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn run_migrations_creates_favorite_folders_table() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'favorite_folders'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn run_migrations_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        // 2回目の呼び出しでも二重にテーブルが作成されてエラーにならないこと
        run_migrations(&conn).unwrap();
    }
}
