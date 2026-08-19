//! クリップボード監視（F-11以降）はPhase2で実装する。
//!
//! architecture.md 1章のディレクトリ構成に合わせてディレクトリのみ先行して用意している。
//! Phase1では中身は未着手であり、`lib.rs`からも`mod clipboard;`宣言はしていない
//! （タスク指示: 「clipboard/モジュールはディレクトリだけ作ってもよいが、中身は着手不要」）。
//!
//! Phase2実装時は、architecture.md 4.1章のtrait設計（`ClipboardWatcher`）に従い、
//! `windows.rs` / `macos.rs` / `dev_stub.rs`へOS分岐する想定。
