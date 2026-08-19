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

### 8.3.1 asset protocolのスコープ設計（セキュリティ）

F-07（プレビュー）実装で画像をwebviewから直接読み込むためにTauriのasset protocolを使うが、シェルフアイテムは任意パス（ユーザーがドラッグした場所ならどこでも）を参照しうるため、静的な許可リストで事前に絞り込めない。ここで`tauri.conf.json`の`assetProtocol.scope`に`"**"`のような包括パターンを置くと、webviewから任意のローカルファイルパスをasset protocol経由で読めるようになってしまい、攻撃対象範囲（アプリ自体は個人利用のローカルデスクトップアプリでリモートコンテンツを読み込まないため実害は限定的だが、防御的に避けるべき）が不必要に広がる。

そのため、実際にシェルフへ追加された実ファイルのパスのみを`tauri::Manager::asset_protocol_scope().allow_file(path)`で**都度動的に許可**する設計を採用する（起動時に既存アイテム分を一括許可、`shelf_add_paths`実行時に新規追加分を許可）。フォルダはプレビュー対象外なので許可しない。`tauri.conf.json`側の静的スコープはクリップボード画像キャッシュ（`$APPDATA/clipboard-cache/*`、アプリ自身が書き込む既知の場所）のみに限定する。

### 8.3 F-06（ロック）・F-07（プレビュー）

- F-06: `shelf_items.locked`カラム（Phase1で先行して用意済み）をトグルする`shelf_set_locked`コマンドを追加。`shelf_clear`の`exclude_locked=true`呼び出しをフロントのデフォルト動作にする（Phase1では常に`false`で呼んでいたのを見直す）
- F-07: ホバーまたはショートカットでプレビューポップオーバーを表示する。画像ファイルはTauriのasset protocol経由で実画像をプレビュー、それ以外のファイルはファイル名・サイズ・更新日時・拡張子ベースの汎用アイコンを表示する簡易実装とする（OSネイティブのサムネイル/アイコン抽出APIは今回のスコープ外とする簡略化判断）

### 8.4 F-19（自動起動）

- `tauri-plugin-autostart`を使用し、設定画面のトグルで有効/無効を切り替える

---

## 9. Phase4 追加設計（F-14, F-15, F-17）

### 9.1 F-14（検索）

Phase2で`clipboard_list_history(query)`は既にLIKE検索を受け付けているが、`%`/`_`のエスケープをしていない既知の課題がある（`docs/architecture.md`旧版・実装報告に記載）。Phase4でこれを解消する: ユーザー入力中の`%`/`_`/`\`をエスケープした上で`LIKE ?1 ESCAPE '\'`を使う。検索対象は`text_content`と`file_paths_json`（Phase2と同様）に加え、Phase4で追加する`tags.name`とのJOINも対象にしてよい（タグ名検索）。

### 9.2 F-15（スタック/マージ）

`clipboard_history`の意味論として「結合＝新しいテキストアイテムの生成」とする（`architecture.md` 2章末尾に既述）。対象は`content_type = 'text'`のアイテムのみ（画像・ファイルパスは対象外。UIの複数選択モードでは選択不可にする）。選択順（一覧での表示順、ピン留め優先ソート後の順）で改行区切りに結合し、新規テキストエントリとして`clipboard_repo::record_entry`相当の経路でINSERTする。元の個別アイテムは削除しない（結合はコピーであり移動ではない、という判断。要件に明記が無いための裁量）。コマンド名: `clipboard_stack_entries(ids: number[]) -> ClipboardEntry`。

### 9.3 F-17（タグ/カテゴリ）

`tags` / `clipboard_tags`テーブルは既にDDLに存在する（`architecture.md` 2章）。Phase4で以下のコマンドを追加する:

| コマンド | 引数 | 説明 |
|---|---|---|
| `tags_list` | - | タグ一覧取得 |
| `tags_create` | `name, color` | タグ作成（name UNIQUE制約違反は`ShelfError`で返す） |
| `tags_delete` | `id` | タグ削除（`clipboard_tags`はON DELETE CASCADEなので関連付けも自動削除） |
| `clipboard_set_tags` | `id, tagIds: number[]` | 指定エントリのタグ付けを一括置き換え（差分diffではなく全置換が実装しやすくバグりにくいための判断） |

一覧取得（`clipboard_list_history`）にタグによるフィルタ引数（`tagId?: number`）を追加する。色は自由入力のhexカラーコード文字列（`#RRGGBB`）とし、プリセットパレットはフロント側でUIの利便性として提示するだけでよい（DB制約は設けない）。

---

## 10. Phase5 追加設計（F-08(Mac), F-22）

### 10.1 F-08（macOS側ドラッグ開始検知）

`requirements.md` 8章の通り、macOSのグローバルなドラッグ開始検知はAccessibility API（正確には「入力監視」権限）またはprivate API依存の可能性が高いとされていた懸念事項について調査・設計した結果を記す。

- **採用方式**: `NSEvent.addGlobalMonitorForEvents(matching:handler:)`によるグローバルマウスイベント監視。`.leftMouseDown` / `.leftMouseDragged` / `.leftMouseUp`を監視し、Windows版（`drag_watch/windows.rs`のWH_MOUSE_LLヒューリスティック）と同じロジック（左ボタン押下位置から一定距離以上の移動が継続したら「ドラッグ開始」とみなす）を踏襲する。private APIには依存しない
- **権限要件**: `NSEvent`のグローバルモニタは、自プロセスにフォーカスが無い他アプリのイベントを監視するため、**「アクセシビリティ」権限**（`System Settings > Privacy & Security > Accessibility`）の許可が必要になる（macOS本体のAPI仕様上の制約であり、実装方式を変えても回避できない）。これはPrivate API依存ではなく公開APIの正規の権限モデルであるため、Mac App Store外の直接配布（`requirements.md` 8章で既に前提としている方針）であれば実装上の制約にはならない
- **初回起動時のUX**: 権限が無い状態では`NSEvent`のグローバルモニタはイベントを一切受け取れない（エラーにはならず単に発火しない）ため、初回起動時に権限が付与されているかを`AXIsProcessTrusted()`で確認し、無ければ設定画面へ誘導するアラートを表示する設計とする（Windows版には無い、macOS固有の追加UI）
- **未検証である旨**: 本リポジトリの開発コンテナはLinuxのためこのコードはコンパイル対象外であり、実機（macOS）での動作確認ができていない。Windows版と同様、静的レビューのみで実装する

### 10.2 F-22（デバイス間同期）— Bring Your Own Firebase方式

`requirements.md` D-7の通り、同期方式は決定済み。開発者（shiki）側がサーバー費用を負担せず、かつユーザー側も無料で運用できることを最優先条件とし、**各ユーザーが自分自身のFirebaseプロジェクト（Sparkプラン＝無料枠）を用意し、その接続情報をLedgeの設定画面に入力する「Bring Your Own Firebase」方式**を採用する。同期は完全にオプトインとし、未設定の場合は従来どおりローカルSQLiteのみで完結する（この場合Phase1〜4の挙動に一切影響しない）。

#### 前提としてユーザーが行うセットアップ（アプリ外の作業）

1. [Firebase Console](https://console.firebase.google.com/)で新規プロジェクトを作成する（Sparkプラン＝無料）
2. Firestore Database を有効化する（本番モードで開始し、後述のセキュリティルールを設定）
3. Authentication を有効化し、Email/Password プロバイダをONにする
4. Authentication上で同期用のアカウントを1つ作成する（同一アカウントを同期させたい全端末で使い回す）
5. プロジェクト設定からWebアプリの構成情報（`apiKey` / `authDomain` / `projectId` / `appId`）を取得する

このセットアップ手順は、実装が完了した段階でアプリ内ヘルプまたは`docs/`配下に別途手順書として用意する（本設計では対象外）。

#### なぜEmail/Password認証か

Firestoreのセキュリティルールで「本人のデータにしかアクセスできない」を強制するには、端末をまたいで同一の`uid`が得られる認証が必要。匿名認証（Anonymous Auth）は端末ごとに異なる`uid`が発行されるため同期の用をなさない。カスタムトークン方式はトークン発行用のサーバーが必要になり「サーバー費用ゼロ」の前提と矛盾する。Email/Password認証はサーバーレスで実現でき、かつユーザーが管理画面上で直接アカウントを作れるため、この制約下で最も単純な選択肢として採用する（将来的にGoogleサインイン等への拡張は妨げない）。

#### データモデル（Firestore）

```
users/{uid}/clipboard_history/{contentHash} … クリップボード履歴のうち「ピン留め済み・かつテキストのみ」を同期対象とする
```

- クリップボード履歴は全件同期すると量・プライバシー面のリスクが大きいため、**ピン留め済み（F-13）かつ`content_type = 'text'`のみ**を同期対象とする。画像（`image`）・ファイルパス（`file_paths`）のピン留めは同期対象外とする（後述の理由によりshelf_itemsと同じ「実体が端末依存」の問題を抱えるため）
- Firestoreのドキュメント ID には、クリップボード側に既存の`content_hash`カラム（重複排除用、`UNIQUE`制約済み）をそのまま流用する。新規のUUID発行やSQLiteスキーマ変更をせずに、同一内容のテキストは自然に同じドキュメントへマージされる
- `clipboard_history`は既に`updated_at`カラムを持つため、そのままFirestore側の`updatedAt`フィールドに対応させる（新規カラムは不要）

**shelf_items（シェルフのファイル/フォルダ）は今回スコープ外とする（当初案から変更）。**
検討の結果、シェルフの各アイテムは`source_path`（例: `/Users/shiki/Downloads/report.pdf`）というその端末のファイルシステムに閉じたパスを本質的な内容として持ち、ファイル実体そのものは同期しない設計（上記の通り）。そのため仮にメタデータだけをFirestoreへ同期しても、同期先の端末にはドラッグアウト（F-03）もプレビュー（F-07）もできない「参照だけのダミーアイテム」が並ぶだけになり、実用上の価値が薄い。無理に実装するとUI上「同期されたが操作できないアイテム」という分かりにくい状態を作り込むことになるため、**shelf_itemsの同期はいったん実装しない**。将来的に必要になった場合は「同期元と同期先が同じアイテムを指しているかのように見せる」のではなく、「他端末のシェルフ内容を参照専用リストとして表示する」等、UI設計から見直す前提とする。

#### 同期戦略

- 個人利用が前提で同時編集の衝突は稀という想定のもと、**Last-Write-Wins**（`updatedAt`が新しい方を採用）とする。複雑な差分マージ・3-way mergeは行わない
- クラウド→ローカル: Firestoreの`onSnapshot`によるリアルタイムリスナーで`users/{uid}/clipboard_history`コレクションの変更を購読し、ローカルSQLiteへTauriコマンド経由で反映する
- ローカル→クラウド: ピン留めテキストの追加/変更/解除等のイベント発生時に、その時点でのピン留め済みテキスト一覧を取得し、前回同期時点の一覧との差分（増えた/変わった分はupsert、消えた分はdelete）をFirestoreへ書き込む。差分計算の基準となる「前回同期した`content_hash`集合」は`tauri-plugin-store`（`settings.json`とは別キー）にローカルキャッシュとして保持する
- 削除の扱い: クラウド側でドキュメントが消えたことを検知した場合、ローカル側は該当エントリを即削除はせず**ピン留めを解除する**（`pinned = false`）に留める。同期の一時的な不整合やネットワーク瞬断で誤って削除が伝播した場合に、実データ自体は失われないようにするための安全側の設計判断

#### 実装配置

同期ロジック本体（Firestoreへのpush/pull・リアルタイムリスナー）はOS非依存のため、Rust側（`src-tauri/`）ではなくフロントエンド（`src/`, TypeScript）に置く。Firebase Web SDK（`firebase`パッケージの`firebase/app` `firebase/auth` `firebase/firestore`、モジュラーAPI）をTauriのWebViewから直接利用する。

ただし、サインイン用パスワードを`AppSettings`（`get_settings`がまるごとフロントへ返す構造体）の外に置く目的で、書き込み専用のTauriコマンド`sync_set_firebase_password`（`src-tauri/src/commands/settings.rs`）を1つ追加している。これは同期ロジックそのものではなく、既存の設定永続化の枠組み（`tauri-plugin-store`）にもう1キー追加するだけの薄いRust側変更であり、上記「フロントに置く」方針とは矛盾しない。

Firebase構成（`apiKey`等）とサインイン用のEmail/Passwordは、既存の`tauri-plugin-store`（`settings.json`）に保存する。これは既存の他の設定項目と同じ保存先・同じ平文JSONであり、セキュリティレベルは既存実装と同水準（ローカル端末上のファイルであり、リモートへは送信しない）。パスワードの秘匿化（OSキーチェーン連携等）は将来の改善課題とし、初期実装のスコープには含めない。

##### 認証セッションの持ち方（設定UI実装時からの変更点）

設定UIの疎通確認（`testFirebaseConnection`、`src/lib/sync/firebase.ts`）は、確認のたびにユニーク名の一時的なFirebaseAppを作って確認後に破棄する実装だった。しかし実際の同期エンジンは常駐して`onSnapshot`を張り続ける必要があり、アプリ再起動のたびにパスワード入力を求めるのは非現実的（パスワードは書き込み専用コマンドで保存するのみで、`get_settings`からは読み出せない設計にしているため、そもそも再入力なしでは再サインインできない）。

そこでFirebase Auth SDKの既定の永続化（`browserLocalPersistence`。TauriのWebViewはブラウザ相当のlocalStorage/IndexedDBを持つため、サインイン状態はアプリ再起動後も保持される）を利用し、**「接続テスト」に成功した時点のサインインをそのまま同期エンジンが使う常駐セッションとする**方式に変更する。具体的には固定名（例: `"ledge-sync"`）のFirebaseAppを1つだけ持ち、`deleteApp`はしない。次回起動時は、`sync_enabled`が有効かつ永続化されたセッションが残っていれば無操作で同期を再開できる。セッションが失効している場合（ブラウザデータの削除等、稀なケース）のみ、設定画面でのパスワード再入力を求める。

#### 同期対象コマンド（追加が必要なもの）

- `clipboard_list_history`（既存, `src-tauri/src/commands/clipboard.rs:17`）: 追加変更なし。同期エンジンはこの既存コマンドで全件取得し、`contentType === 'text' && pinned === true`をフロント側でフィルタしてpush対象を決める（Rust側のクエリ条件を増やすほどの件数規模ではないため）
- `clipboard_sync_upsert_from_cloud`（新規, Rust）: `{ contentHash, textContent, updatedAt }`を受け取り、`content_hash`一致の既存行があれば`updatedAt`を比較してクラウド側が新しい場合のみ`text_content`/`pinned=true`/`updated_at`を更新、既存行が無ければ`content_type='text'`のピン留め済み新規行として挿入する。既存の`clipboard_repo::record_entry`とは「常にpinned扱いで挿入する」「タイムスタンプをクラウド側の値で上書きする」点が異なるため、`record_entry`を流用せず専用関数として`clipboard_repo.rs`に追加する
- `clipboard_sync_unpin_by_hash`（新規, Rust）: `{ contentHash }`を受け取り、該当行があれば`pinned = false`に更新する（無ければ何もしない）。クラウド側でのドキュメント削除をローカルへ反映する経路

#### Firestoreセキュリティルール（設定例）

```
rules_version = '2';
service cloud.firestore {
  match /databases/{database}/documents {
    match /users/{uid}/{document=**} {
      allow read, write: if request.auth != null && request.auth.uid == uid;
    }
  }
}
```

#### スコープ外（将来検討）

- shelf_items（シェルフのファイル/フォルダ）の同期。上記の通りファイルシステムパスが端末依存であるため実用上の価値が薄く、実装しない（UI設計を含めた見直しが必要）
- クリップボード履歴のうち画像（`image`）・ファイルパス（`file_paths`）のピン留めの同期。画像キャッシュやファイルパスも端末依存のため対象外
- ファイル本体そのものの同期（Firebase Storage等の追加が必要）
- E2E暗号化（Firestore側管理者からは平文で見える。個人の1アカウント運用が前提のため初期スコープには含めない）
- 複数アカウント/共有機能

---

## 11. フェーズと本ドキュメントの対応

CLAUDE.mdのフェーズ一覧・機能IDとの対応は`requirements.md` 7章の通り。本ドキュメントの各セクションはPhase1〜4を横断して先取りした設計になっているが、実装はフェーズ順に行い、未着手フェーズのテーブル/コマンドはマイグレーション・コード上に用意しても機能としては呼び出さない（UIから到達不可にする）。
