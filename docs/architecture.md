# アーキテクチャ設計書

`requirements.md`（仕様のSSoT）を実装可能な単位まで分解したもの。仕様と実装方針が食い違う場合は`requirements.md`を優先し、本ドキュメントを追従修正する。

---

## 1. ディレクトリ構成

CLAUDE.mdの想定構成を実装レベルまで具体化したもの。

```
.
├── src/                            # Svelte 5 + TS フロントエンド
│   ├── lib/
│   │   ├── components/
│   │   │   ├── Shelf.svelte            # シェルフ本体（一覧・D&D領域）
│   │   │   ├── ShelfItem.svelte        # シェルフ内の1アイテム表示
│   │   │   ├── ClipboardHistory.svelte # Phase2
│   │   │   ├── ClipboardItem.svelte    # Phase2
│   │   │   └── Settings.svelte         # Phase3
│   │   ├── stores/
│   │   │   ├── shelfStore.ts
│   │   │   ├── clipboardStore.ts       # Phase2
│   │   │   └── settingsStore.ts
│   │   ├── api/
│   │   │   └── commands.ts             # invoke()の型付きラッパー。Rust側コマンド名と1:1対応
│   │   └── types/
│   │       ├── shelf.ts
│   │       ├── clipboard.ts            # Phase2
│   │       └── settings.ts
│   ├── App.svelte                      # シェルフウィンドウのエントリ
│   └── main.ts
├── src-tauri/
│   ├── src/
│   │   ├── main.rs                     # エントリポイントのみ。実体はlib.rsに委譲
│   │   ├── lib.rs                      # tauri::Builder組み立て、プラグイン登録、run()
│   │   ├── commands/                   # #[tauri::command]ハンドラ（薄い層。ロジックはdomain側に置く）
│   │   │   ├── mod.rs
│   │   │   ├── shelf.rs
│   │   │   ├── clipboard.rs            # Phase2
│   │   │   └── settings.rs
│   │   ├── clipboard/                  # OS分岐の監視ロジック
│   │   │   ├── mod.rs                  # trait ClipboardWatcher定義
│   │   │   ├── windows.rs              # #[cfg(target_os = "windows")]
│   │   │   ├── macos.rs                # #[cfg(target_os = "macos")]
│   │   │   └── dev_stub.rs             # #[cfg(not(any(windows, macos)))] Linux開発検証用no-op実装
│   │   ├── drag_drop/                  # アウトバウンドD&D（F-03）
│   │   │   ├── mod.rs                  # trait DragOutSource定義
│   │   │   ├── native.rs               # windows/macos共通: tauri-plugin-dragベースの実装
│   │   │   └── dev_stub.rs
│   │   ├── storage/
│   │   │   ├── mod.rs
│   │   │   ├── db.rs                   # コネクション管理・マイグレーション実行
│   │   │   ├── models.rs               # ShelfItem等のRust構造体（Serialize/Deserialize）
│   │   │   ├── shelf_repo.rs
│   │   │   └── clipboard_repo.rs       # Phase2
│   │   ├── settings/
│   │   │   └── mod.rs                  # tauri-plugin-store経由のAppSettings読み書き
│   │   ├── shortcut/
│   │   │   └── mod.rs                  # 設定値からグローバルショートカットを登録/再登録
│   │   ├── tray.rs                     # トレイアイコン・メニュー
│   │   └── error.rs                    # ShelfError（thiserror）
│   ├── migrations/
│   │   └── 0001_init.sql
│   ├── icons/
│   ├── Cargo.toml
│   ├── build.rs
│   └── tauri.conf.json
├── docs/
│   ├── requirements.md
│   └── architecture.md
├── package.json
├── vite.config.ts
├── svelte.config.js
├── tsconfig.json
├── CLAUDE.md
└── README.md
```

**方針**: OS依存コードは`clipboard/`と`drag_drop/`にのみ存在させ、trait越しに`commands/`から呼び出す。Windows/macOS以外（開発検証用のLinux）では`dev_stub.rs`のno-op実装を使い、`cargo check`/`cargo build`をこの開発コンテナ上でも通せるようにする。ただし製品としての配布ターゲットはWindows/macOSのみ（`requirements.md` 9章）。

---

## 2. データモデル（SQLite DDL）

`requirements.md` 10章の意味論に対応する実装。マイグレーションは`src-tauri/migrations/0001_init.sql`に配置し、起動時に`rusqlite_migration`（またはアプリ内の簡易マイグレーションランナー）で適用する。

```sql
-- シェルフアイテム（Phase1〜）
CREATE TABLE shelf_items (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    item_type     TEXT NOT NULL CHECK (item_type IN ('file', 'folder')),
    source_path   TEXT NOT NULL,
    display_name  TEXT NOT NULL,
    size_bytes    INTEGER,
    locked        INTEGER NOT NULL DEFAULT 0,   -- F-06 (Phase3)
    sort_order    INTEGER NOT NULL,
    added_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_shelf_items_sort_order ON shelf_items(sort_order);

-- クリップボード履歴（Phase2〜）
CREATE TABLE clipboard_history (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    content_type    TEXT NOT NULL CHECK (content_type IN ('text', 'image', 'file_paths')),
    text_content    TEXT,
    image_path      TEXT,          -- アプリデータディレクトリ配下のPNGファイルパス（BLOBはDBに入れない）
    thumbnail_path  TEXT,
    file_paths_json TEXT,          -- content_type='file_paths'の場合のJSON配列
    content_hash    TEXT NOT NULL, -- 重複排除キー
    pinned          INTEGER NOT NULL DEFAULT 0,  -- F-13 (Phase2)
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE UNIQUE INDEX idx_clipboard_content_hash ON clipboard_history(content_hash);
CREATE INDEX idx_clipboard_created_at ON clipboard_history(created_at);

-- タグ（Phase4, F-17）
CREATE TABLE tags (
    id    INTEGER PRIMARY KEY AUTOINCREMENT,
    name  TEXT NOT NULL UNIQUE,
    color TEXT
);
CREATE TABLE clipboard_tags (
    clipboard_id INTEGER NOT NULL REFERENCES clipboard_history(id) ON DELETE CASCADE,
    tag_id       INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (clipboard_id, tag_id)
);

-- スタック（Phase4, F-15）: クリップボード項目の結合はDB上は独立エンティティとして新規text履歴を1件作る方式とし、
-- 専用テーブルは持たない（結合＝新しいテキストアイテムの生成、という単純なモデルにする）
```

設定（ホットキー・表示位置/透明度・自動クリア条件・自動起動ON/OFF）はSQLiteではなく`tauri-plugin-store`による`settings.json`で管理する（`requirements.md` 10.4）。

---

## 3. Tauriコマンド一覧（IPC境界）

フロントエンドは`src/lib/api/commands.ts`経由でのみRust側を呼び出す。コマンド名はRust側の関数名と1:1。

### Phase 1
| コマンド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `shelf_list_items` | - | `ShelfItem[]` | 起動時・表示時の一覧取得 |
| `shelf_add_paths` | `paths: string[]` | `ShelfItem[]` | D&Dで受け取ったパスを格納（追加分を返す） |
| `shelf_remove_item` | `id: number` | `void` | 個別削除 |
| `shelf_clear` | `excludeLocked: boolean` | `void` | 一括削除（F-06実装後はロック対象を除外） |
| `shelf_begin_drag_out` | `ids: number[]` | `void` | シェルフ→外部アプリへのネイティブドラッグ開始（F-03） |
| `get_settings` | - | `AppSettings` | 設定取得 |
| `update_settings` | `patch: Partial<AppSettings>` | `AppSettings` | 設定更新（ホットキー再登録・ウィンドウ再配置をトリガ） |

### Phase 2
| コマンド | 引数 | 戻り値 | 説明 |
|---|---|---|---|
| `clipboard_list_history` | `query?: string` | `ClipboardEntry[]` | 履歴取得（F-14検索は簡易LIKE検索から） |
| `clipboard_paste_to_active` | `id: number` | `void` | クリップボードへ書き戻し（F-12） |
| `clipboard_set_pinned` | `id: number, pinned: boolean` | `void` | ピン留め（F-13） |
| `clipboard_delete` | `id: number` | `void` | 個別削除 |
| `clipboard_clear` | `excludePinned: boolean` | `void` | 一括削除 |

Rust側からフロントへのプッシュ通知は Tauri のイベント (`app_handle.emit`) を使用する：`shelf://items-changed`、`clipboard://history-changed` など。DBを更新した全コマンドは処理末尾でこれをemitし、フロントは購読して自動再取得する（ポーリング不要）。

---

## 4. OS抽象化 trait 設計

### 4.1 クリップボード監視

```rust
// src-tauri/src/clipboard/mod.rs
pub enum ClipboardSnapshot {
    Text(String),
    Image(Vec<u8>),       // PNGエンコード済みバイト列
    FilePaths(Vec<PathBuf>),
}

pub trait ClipboardWatcher: Send {
    /// 監視を開始し、変更検知のたびにコールバックを呼ぶ。
    /// 除外規約（requirements.md 10.3）のチェックは実装側（各OS impl）で行い、
    /// 除外対象はコールバックを呼ばずに握りつぶす。
    fn start(&mut self, on_change: Box<dyn Fn(ClipboardSnapshot) + Send + Sync>) -> Result<(), ShelfError>;
    fn stop(&mut self);
}
```

- **Windows実装**: `AddClipboardFormatListener`によるイベント駆動（メッセージ専用ウィンドウを1つ作成し`WM_CLIPBOARDUPDATE`を受信）。`ExcludeClipboardContentFromMonitorProcessing`フォーマットの有無をチェックして除外
- **macOS実装**: `NSPasteboard.general.changeCount`のポーリング（既定400ms間隔。シェルフ/履歴パネルが非表示中はバッテリー消費を抑えるため800ms〜1000ms間隔に緩和する適応的間隔を採用）。`org.nspasteboard.ConcealedType`等の型チェックで除外
- **dev_stub実装**: Linux開発環境ではclipboard-rs/arboard経由の簡易ポーリング実装 or 完全no-op（型チェック通過のみが目的のため、当面no-opで十分）

### 4.2 アウトバウンドドラッグ（F-03）

```rust
// src-tauri/src/drag_drop/mod.rs
pub trait DragOutSource: Send {
    fn begin_drag(&self, paths: Vec<PathBuf>) -> Result<(), ShelfError>;
}
```

- **実装方針（技術選定の補足）**: `requirements.md` 5章は「windows-rs（COM/IDropTarget）」「NSPasteboard/NSDraggingSession」を明記しているが、これをフルスクラッチで実装するとCOMのIDropSource/IDataObject実装やNSDraggingSource準拠のObjective-C連携など、実装コストとバグ面のリスクが大きい。そこで**`tauri-plugin-drag`（crabnebula製）を第一候補として採用**する。内部的には同じOS APIを使っており、要件の技術選定と矛盾しない。trait越しに呼んでいるため、将来カスタムのドラッグプレビュー画像合成等が必要になれば`native.rs`の中身だけ差し替えれば良い
- 受け入れ側（F-02のシェルフへのドロップ）はTauri v2の組み込みドラッグ&ドロップイベント（`dragDropEnabled` + `onDragDropEvent`）で完結し、独自のCOM/NSPasteboardコードは不要

---

## 5. ウィンドウ / トレイ / グローバルホットキー

- **シェルフウィンドウ**: `decorations: false`, `transparent: true`, `alwaysOnTop: true`, `skipTaskbar: true`, `resizable: false`（Phase1）。既定サイズ幅300px・高さはプライマリディスプレイの作業領域に追従。表示位置は`settings.edge`（top/bottom/left/right、既定は`right`）と`settings.monitor`（既定はプライマリ）から算出し、表示のたびに再計算する
- **透明度（F-10, Phase3）**: ウィンドウ全体ではなく背景要素にCSSの`opacity`/`backdrop-filter`を適用し、ドラッグ中のアイテムやテキストの視認性を保つ
- **トレイアイコン**: `TrayIconBuilder`で常駐。左クリックでシェルフの表示/非表示トグル、右クリックメニューは「シェルフを表示」「設定...」（Phase1では無効化しPhase3で有効化）「終了」
- **グローバルホットキー**: `tauri-plugin-global-shortcut`。起動時に`settings.json`から読み込んで登録し、設定変更時は一度全解除してから再登録する。既定値は`requirements.md` D-5参照（暫定: シェルフ`Ctrl+Alt+S`/`Cmd+Option+S`、履歴`Ctrl+Alt+V`/`Cmd+Option+V`）
- **多重起動防止**: `tauri-plugin-single-instance`で2重起動時は既存インスタンスにフォーカス（またはシェルフをトグル）させる
- **自動起動（F-19, Phase3）**: `tauri-plugin-autostart`

---

## 6. エラー処理・状態同期

- `ShelfError`（`thiserror`）: `#[error("...")]`のバリアントごとにユーザー向けメッセージを持たせ、`impl Serialize`してフロントへそのまま渡す
- 全コマンドは`Result<T, ShelfError>`を返し、フロント（`commands.ts`）は失敗時にトースト表示。個別コマンドでのエラーはアプリ全体をクラッシュさせない
- DB更新を伴うコマンドは完了後に`emit`でフロントへ通知し、フロントは該当ストアを再取得する（3章末尾参照）

---

## 7. 開発時のビルド検証について

このリポジトリの開発コンテナ（Linux/Ubuntu）にはWindows/macOS向けのクロスコンパイル環境が無いため、`#[cfg(target_os = "windows")]`/`#[cfg(target_os = "macos")]`配下のコードはこの環境ではコンパイルされない。`dev_stub`実装により`cargo check --target x86_64-unknown-linux-gnu`（デフォルトターゲット）で共通部分（storage/commands/settings/tray等）の型検査とロジックのテストは可能にしておく。Windows/macOS固有コード自体の動作確認は実機（またはCI上のクロスプラットフォームランナー）が必要であり、本セッションでは静的なレビューとロジックの妥当性検証に留める。

---

## 8. Phase3 追加設計（F-06, F-07, F-08(Win), F-10, F-19）

### 8.1 F-08（Windows側ドラッグ開始検知によるシェルフ自動表示）

Windowsには「OS全体でドラッグ操作が始まったこと」を通知する高レベルAPIが存在しない（OLEのドラッグはドロップターゲット側でしか検知できない）。そのため、`WH_MOUSE_LL`（低レベルマウスフック）を使い、「左ボタン押下→一定距離以上の移動が継続」というパターンをヒューリスティックに「ドラッグ操作の可能性が高い」と判定し、シェルフを自動表示する方式を採る。

- 誤検知（ファイルを伴わない単なるドラッグ選択やウィンドウ移動でもシェルフが出る）は避けられないが、シェルフの自動表示自体は非破壊的操作（表示されるだけで何かが起きるわけではない）なので実害は小さいと判断する
- ユーザー体験への影響を考慮し、設定でON/OFFできるようにする（`AppSettings.auto_show_on_drag_start`、既定値`true`）。誤検知が気になる場合はOFFにできる
- 判定ロジック: マウス左ボタン押下位置から一定距離（既定8px、DPI非依存の論理px換算）以上移動した状態が続いたらシェルフを表示。ボタンを離す（`WM_LBUTTONUP`相当）か、一定時間（既定800ms）操作が無ければ自動的に表示状態を解除して良い（ユーザーが明示的にシェルフを開いていた場合はこの自動非表示の対象外にする＝ホットキーで開いた状態と自動表示状態を区別するフラグを持つ）
- 実装は`WH_MOUSE_LL`フック用のメッセージ専用スレッド上で行い、UIスレッドをブロックしないようチャネル経由でメインスレッドへ通知する
- macOS側（F-08 Mac）はPhase5で別途検討する（Accessibility API依存が濃厚なため要調査。本フェーズでは着手しない）

### 8.2 F-10（表示位置/透明度設定）のスコープ

- 対応するのは「画面端（上下左右）」の切り替えと「透明度（0.0〜1.0）」のみとする。マルチモニタでの表示先モニタ選択は複雑さの割に個人利用での価値が低いと判断し、Phase3では「プライマリディスプレイ固定」のまま据え置く（将来必要になれば設定に`monitor`項目を追加する形で拡張可能なようAppSettings側は既に`monitor`フィールドを持たせておいてよい）
- 設定変更は即座にウィンドウへ反映する（`update_settings`コマンド内で位置・透明度の再計算をトリガー）

### 8.3 F-06（ロック）・F-07（プレビュー）

- F-06: `shelf_items.locked`カラム（Phase1で先行して用意済み）をトグルする`shelf_set_locked`コマンドを追加。`shelf_clear`の`exclude_locked=true`呼び出しをフロントのデフォルト動作にする（Phase1では常に`false`で呼んでいたのを見直す）
- F-07: ホバーまたはショートカットでプレビューポップオーバーを表示する。画像ファイルはTauriのasset protocol経由で実画像をプレビュー、それ以外のファイルはファイル名・サイズ・更新日時・拡張子ベースの汎用アイコンを表示する簡易実装とする（OSネイティブのサムネイル/アイコン抽出APIは今回のスコープ外とする簡略化判断）

### 8.4 F-19（自動起動）

- `tauri-plugin-autostart`を使用し、設定画面のトグルで有効/無効を切り替える

---

## 9. フェーズと本ドキュメントの対応

CLAUDE.mdのフェーズ一覧・機能IDとの対応は`requirements.md` 7章の通り。本ドキュメントの各セクションはPhase1〜4を横断して先取りした設計になっているが、実装はフェーズ順に行い、未着手フェーズのテーブル/コマンドはマイグレーション・コード上に用意しても機能としては呼び出さない（UIから到達不可にする）。
