# Shelf Drop（仮称）

Windows / macOS 両対応のファイル一時置き場（シェルフ）＋クリップボード履歴管理アプリ。

Windows向けクリップボードマネージャー「Edge Drop」、Mac向けシェルフアプリ「Yoink」「Dockside」を参考に、
両OSに対応した無料・オープンソースのファイル/クリップボード一時置き場アプリを目指す個人プロジェクト。

## できること（予定）

- 画面端常駐の「シェルフ」にファイル/フォルダをドラッグ&ドロップして一時的に置き、任意のアプリへ再ドロップする
- テキスト/画像/ファイルパスのクリップボード履歴を自動記録し、ピン留め・検索する
- グローバルホットキーでシェルフ・履歴を呼び出す
- Windows / macOS で同一の操作感を実現する

詳細な機能一覧・優先度は [docs/requirements.md](docs/requirements.md) を参照。

## 技術スタック

| 層 | 採用技術 |
|---|---|
| フロントエンド | Svelte 5 + TypeScript |
| バックエンド | Rust（Tauri v2） |
| データ永続化 | SQLite（rusqlite） |
| クリップボード抽象化 | arboard または clipboard-rs |
| グローバルホットキー | tauri-plugin-global-shortcut |
| Windows D&D | windows-rs（COM / IDropTarget） |
| macOS D&D | NSPasteboard / NSDraggingSession（Tauri経由） |

## 現在の状態

**2026-08-19時点、実装未着手（Phase 0）。** 要件定義書とプロジェクト方針のみ存在し、
`src/` / `src-tauri/` はまだ作成されていない。

実装フェーズの進捗は [CLAUDE.md](CLAUDE.md) のチェックリストで管理する。

- [ ] Phase 1（MVP）: 常駐シェルフ + ファイルD&D格納
- [ ] Phase 2: クリップボード履歴 + ピン留め
- [ ] Phase 3: Windows側自動検出、表示設定、自動起動
- [ ] Phase 4: 検索・タグ・スタック
- [ ] Phase 5: macOS側自動検出、同期検討

## 開発コマンド（seed、実装開始時に更新）

```bash
# 開発サーバー起動
npm run tauri dev

# ビルド
npm run tauri build
```

## ドキュメント

- [docs/requirements.md](docs/requirements.md) — 要件定義書（仕様のSSoT）
- [CLAUDE.md](CLAUDE.md) — Claude Codeによる実装作業のためのプロジェクトコンテキスト
