//! ファイル/フォルダをZIP圧縮するユーティリティ（Phase6, F-21「圧縮してシェルフに追加」）。
//!
//! DBアクセスを含まない純粋なファイルシステム処理のため`storage`配下には置かず、
//! トップレベルモジュールとして独立させる（storage/mod.rsの「SQLiteアクセスはこのモジュール配下に
//! 閉じ込める」という方針を守るための切り分け。迷った設計判断: 呼び出し元へ報告）。

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::error::ShelfError;

/// 対象ファイル/フォルダをZIP圧縮し、生成したZIPファイルのパスを返す。
///
/// 圧縮先はOS一時ディレクトリ（`std::env::temp_dir()`）配下にする。元ファイルと同じディレクトリを
/// 使わないのは、読み取り専用ディレクトリ等への書き込み権限エラーを避けるため（architecture.md 12.2章）。
/// フォルダの場合は`display_name`をZIP内のルートディレクトリ名として使い、
/// 展開時に元のフォルダ名で復元できるようにする。
pub fn compress_to_zip(source: &Path, display_name: &str) -> Result<PathBuf, ShelfError> {
    let zip_path = unique_zip_path(display_name);
    let file = File::create(&zip_path).map_err(io_err)?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    if source.is_dir() {
        writer
            .add_directory(format!("{display_name}/"), options)
            .map_err(zip_err)?;
        add_dir_recursive(&mut writer, source, source, display_name, options)?;
    } else {
        writer.start_file(display_name, options).map_err(zip_err)?;
        let bytes = std::fs::read(source).map_err(io_err)?;
        writer.write_all(&bytes).map_err(io_err)?;
    }

    writer.finish().map_err(zip_err)?;
    Ok(zip_path)
}

/// OS一時ディレクトリ配下に一意なZIPファイル名を生成する（同名ファイルの上書き事故を防ぐ）。
fn unique_zip_path(display_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("{display_name}-{nanos}.zip"))
}

/// フォルダの中身を`std::fs::read_dir`で手動再帰し、`{root_name}/相対パス`という
/// エントリ名でZIPへ追加する（依存を増やさない方針、architecture.md 12.2章）。
fn add_dir_recursive<W: Write + io::Seek>(
    writer: &mut ZipWriter<W>,
    base: &Path,
    dir: &Path,
    root_name: &str,
    options: SimpleFileOptions,
) -> Result<(), ShelfError> {
    for entry in std::fs::read_dir(dir).map_err(io_err)? {
        let entry = entry.map_err(io_err)?;
        let path = entry.path();
        let rel_path = path.strip_prefix(base).map_err(|e| {
            ShelfError::Internal(format!("圧縮対象パスの解決に失敗しました: {e}"))
        })?;
        // ZIP内のパス区切りは常に`/`にする（Windows由来の`\`区切りをそのまま使わない）
        let rel_str = rel_path.to_string_lossy().replace('\\', "/");
        let entry_name = format!("{root_name}/{rel_str}");

        if path.is_dir() {
            writer
                .add_directory(format!("{entry_name}/"), options)
                .map_err(zip_err)?;
            add_dir_recursive(writer, base, &path, root_name, options)?;
        } else {
            writer.start_file(entry_name, options).map_err(zip_err)?;
            let bytes = std::fs::read(&path).map_err(io_err)?;
            writer.write_all(&bytes).map_err(io_err)?;
        }
    }
    Ok(())
}

fn io_err(e: std::io::Error) -> ShelfError {
    ShelfError::Internal(format!("ファイルの読み書きに失敗しました: {e}"))
}

fn zip_err(e: zip::result::ZipError) -> ShelfError {
    ShelfError::Internal(format!("圧縮処理に失敗しました: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::io::Read;

    /// テスト用に一意な一時ディレクトリを作る。
    fn make_temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("ledge-compress-test-{name}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// ZIPファイルを展開し、格納されているエントリ名の集合とテキスト内容を返す。
    fn zip_entries(zip_path: &Path) -> (BTreeSet<String>, String) {
        let file = File::open(zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut names = BTreeSet::new();
        let mut first_file_content = String::new();
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).unwrap();
            names.insert(entry.name().to_string());
            if entry.is_file() && first_file_content.is_empty() {
                let _ = entry.read_to_string(&mut first_file_content);
            }
        }
        (names, first_file_content)
    }

    #[test]
    fn compress_single_file() {
        let dir = make_temp_dir("single_file");
        let file_path = dir.join("hello.txt");
        std::fs::write(&file_path, "hello world").unwrap();

        let zip_path = compress_to_zip(&file_path, "hello.txt").unwrap();
        assert!(zip_path.exists());

        let (names, content) = zip_entries(&zip_path);
        assert_eq!(names, BTreeSet::from(["hello.txt".to_string()]));
        assert_eq!(content, "hello world");

        std::fs::remove_file(&zip_path).ok();
    }

    #[test]
    fn compress_folder_recursively() {
        let dir = make_temp_dir("folder");
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        std::fs::write(dir.join("a.txt"), "A").unwrap();
        std::fs::write(dir.join("nested/b.txt"), "B").unwrap();

        let zip_path = compress_to_zip(&dir, "folder").unwrap();
        let (names, _) = zip_entries(&zip_path);

        assert!(names.contains("folder/"));
        assert!(names.contains("folder/a.txt"));
        assert!(names.contains("folder/nested/"));
        assert!(names.contains("folder/nested/b.txt"));

        std::fs::remove_file(&zip_path).ok();
    }
}
