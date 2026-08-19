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

**2026-08-19時点、Phase 5のうち実装可能な範囲（F-08のmacOS側）まで完了。** F-22（デバイス間同期）は
同期方式（自前サーバー/クラウドストレージ経由/エクスポート・インポートのみ、等）自体がユーザー本人の
判断を要する事項のため未着手（`docs/requirements.md` D-7、`docs/architecture.md` 10.2章）。それ以外は
Phase 1〜4に加えて以下の機能を実装済み。

- F-01: グローバルホットキー（既定 `Ctrl+Alt+S` / `Cmd+Option+S`）でシェルフウィンドウの表示/非表示をトグル
- F-02: ファイル/フォルダをシェルフへドラッグ&ドロップして格納（実体コピーはせずパス参照のみ保持）
- F-03: シェルフ内アイテムを外部アプリ/フォルダへドラッグして送り出す（Windows/macOSのみ。`drag`クレートを利用）
- F-04: 複数アイテムの保持・個別操作
- F-05: 個別削除・一括削除
- F-06: ロック機能（ロック中のアイテムは「全て削除」の対象から除外。個別削除は可能）
- F-07: プレビュー表示（ホバーで画像/ファイル情報のポップオーバー表示。画像はasset protocol経由で実画像、
  それ以外は拡張子ベースのアイコン＋ファイル名/サイズ/更新日時）
- F-08: ドラッグ開始のヒューリスティック検知でシェルフを自動表示（設定でON/OFF可能。Windowsは
  `WH_MOUSE_LL`低レベルマウスフック、macOSは`NSEvent`グローバルモニタ（要アクセシビリティ権限）。
  いずれも実機未検証、`docs/architecture.md` 8.1章・10.1章参照）
- F-10: シェルフの表示位置（上下左右）・透明度を設定画面から変更可能（即時反映）
- F-11: テキスト/画像/ファイルパスのクリップボード履歴を自動記録（Windows: `AddClipboardFormatListener`
  イベント駆動、macOS: `NSPasteboard.changeCount`ポーリング。除外規約
  `ExcludeClipboardContentFromMonitorProcessing` / `org.nspasteboard.*` に対応）
- F-12: 履歴一覧からクリップボードへ書き戻す（`arboard`を利用。テキスト/画像に対応、
  ファイルパスの書き戻しは未対応）
- F-13: 履歴アイテムのピン留め（自動削除・一括削除の対象から除外）
- F-16: 自動クリア（既定500件 or 30日、ピン留めアイテムは対象外。設定画面から変更可能）
- F-18: システムトレイ常駐（左クリックでシェルフ表示トグル、右クリックメニューから「設定...」も呼び出し可能）
- F-19: OSログイン時自動起動（`tauri-plugin-autostart`、設定画面からON/OFF）
- F-20: グローバルホットキーの登録を設定値（`settings.json`）駆動にする仕組み。設定画面から変更可能
- F-14: クリップボード履歴の検索（本文/ファイルパス/タグ名に対するLIKE検索。`%`/`_`/`\`はエスケープ済み）
- F-15: 複数のテキストアイテムを選択して改行結合し、新規テキストエントリとして記録するスタック機能
  （元アイテムは削除しない。対象はtext種別のみ）
- F-17: クリップボード履歴アイテムへのタグ付け（作成・削除・付け外し）とタグによる絞り込み表示

シェルフ・クリップボード履歴・設定は1つのウィンドウ内でタブ切り替えする形にしている
（別ウィンドウ化はPhase3で再検討したが、既存のタブ切り替え方式との一貫性を優先した）。
このLinux開発コンテナではWindows/macOS固有コード（`drag_drop/native.rs`、
`clipboard/windows.rs`、`clipboard/macos.rs`、`drag_watch/windows.rs`、`drag_watch/macos.rs`）は
コンパイル対象外のため、静的レビューに留めている（`docs/architecture.md` 7章参照）。特に
`drag_watch/windows.rs`・`drag_watch/macos.rs`は実機での動作確認が未実施。

実装フェーズの進捗は [CLAUDE.md](CLAUDE.md) のチェックリストで管理する。

- [x] Phase 1（MVP）: 常駐シェルフ + ファイルD&D格納
- [x] Phase 2: クリップボード履歴 + ピン留め
- [x] Phase 3: Windows側自動検出、表示設定、自動起動
- [x] Phase 4: 検索・タグ・スタック
- [ ] Phase 5: macOS側自動検出、同期検討（F-08(Mac)実装済み・未検証／F-22はユーザー判断待ちのため未着手）

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
