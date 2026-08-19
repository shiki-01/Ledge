//! SQLiteアクセス層（architecture.md 1章・2章）。
//!
//! ファイル自体は参照のみで実体コピーは行わない（requirements.md 5章・10.1章）。
//! SQLiteへのアクセスはこのモジュール配下に閉じ込め、他モジュールからは
//! `shelf_repo`等のリポジトリ関数越しにのみ触る。

pub mod db;
pub mod models;
pub mod shelf_repo;
