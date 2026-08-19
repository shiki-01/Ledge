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
| Windows D&D | `drag`クレート（tauri-plugin-dragが内部で使用するものと同一。COM/IDropTargetベース） |
| macOS D&D | `drag`クレート（NSPasteboard/NSDraggingSessionベース） |

## 現在の状態

**2026-08-19時点、Phase 1（MVP）完了。** プロジェクトscaffold一式（Svelte 5 + TypeScript + Vite、
Tauri v2 + Rust）を作成し、以下の機能を実装済み。

- F-01: グローバルホットキー（既定 `Ctrl+Alt+S` / `Cmd+Option+S`）でシェルフウィンドウの表示/非表示をトグル
- F-02: ファイル/フォルダをシェルフへドラッグ&ドロップして格納（実体コピーはせずパス参照のみ保持）
- F-03: シェルフ内アイテムを外部アプリ/フォルダへドラッグして送り出す（Windows/macOSのみ。`drag`クレートを利用）
- F-04: 複数アイテムの保持・個別操作
- F-05: 個別削除・一括削除
- F-18: システムトレイ常駐（左クリックでシェルフ表示トグル、右クリックメニュー）
- F-20: グローバルホットキーの登録を設定値（`settings.json`）駆動にする仕組み（カスタマイズUI自体はPhase3）

クリップボード履歴（F-11〜）・設定画面UI・ロック機能（F-06）等はPhase2以降で対応する。
このLinux開発コンテナではWindows/macOS固有コード（`drag_drop/native.rs`）は
コンパイル対象外のため、静的レビューに留めている（`architecture.md` 7章参照）。

実装フェーズの進捗は [CLAUDE.md](CLAUDE.md) のチェックリストで管理する。

- [x] Phase 1（MVP）: 常駐シェルフ + ファイルD&D格納
- [ ] Phase 2: クリップボード履歴 + ピン留め
- [ ] Phase 3: Windows側自動検出、表示設定、自動起動
- [ ] Phase 4: 検索・タグ・スタック
- [ ] Phase 5: macOS側自動検出、同期検討

## 開発コマンド

```bash
# フロントエンドの依存関係をインストール
npm install

# 開発サーバー起動（Tauriアプリとして起動。Windows/macOS環境が必要）
npm run tauri dev

# ビルド
npm run tauri build

# フロントエンドの型チェック（svelte-check）
npm run check

# Rust側の型チェック（src-tauriディレクトリ内で実行）
cd src-tauri && cargo check
```

## ドキュメント

- [docs/requirements.md](docs/requirements.md) — 要件定義書（仕様のSSoT）
- [CLAUDE.md](CLAUDE.md) — Claude Codeによる実装作業のためのプロジェクトコンテキスト
