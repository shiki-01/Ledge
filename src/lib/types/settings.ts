/**
 * シェルフを表示する画面端。Rust側 `settings::ShelfEdge` と対応する。
 */
export type ShelfEdge = "top" | "bottom" | "left" | "right";

/**
 * アプリ設定。Rust側 `settings::AppSettings` とフィールド名を揃えている。
 */
export interface AppSettings {
  shelfHotkey: string;
  shelfEdge: ShelfEdge;
  /** クリップボード履歴の自動クリア（F-16）: 件数上限。既定500件。 */
  clipboardMaxEntries: number;
  /** クリップボード履歴の自動クリア（F-16）: 経過日数上限。既定30日。 */
  clipboardRetentionDays: number;
  /** シェルフ背景の透明度（F-10）。0.0（完全透明）〜1.0（不透明）、既定0.85。 */
  opacity: number;
  /** OS起動時の自動起動ON/OFF（F-19）。既定false。 */
  autostartEnabled: boolean;
  /** ドラッグ開始検知によるシェルフ自動表示のON/OFF（F-08 Windows先行）。既定true。 */
  autoShowOnDragStart: boolean;
  /**
   * デバイス間同期（F-22, Bring Your Own Firebase方式）の有効/無効。既定false。
   * 未設定時は従来どおりローカルSQLiteのみで完結する（architecture.md 10.2章）。
   */
  syncEnabled: boolean;
  /** Firebase Web SDK構成: apiKey。ユーザー自身のFirebaseプロジェクトのものを入力する。 */
  firebaseApiKey?: string;
  /** Firebase Web SDK構成: authDomain。 */
  firebaseAuthDomain?: string;
  /** Firebase Web SDK構成: projectId。 */
  firebaseProjectId?: string;
  /** Firebase Web SDK構成: appId。 */
  firebaseAppId?: string;
  /**
   * Firebase Authentication（Email/Password）のサインイン用Email。
   * パスワードはここに含めない。`get_settings`のレスポンス経路には一切乗せない設計のため、
   * 別コマンド`syncSetFirebasePassword`（`src/lib/api/commands.ts`）で書き込み専用に扱う。
   */
  firebaseEmail?: string;
}

/**
 * Firebase Web SDK初期化に必要な構成情報（`src/lib/sync/firebase.ts`で使用）。
 * `AppSettings`のFirebase系フィールドと同じ内容だが、`initFirebase`/`testFirebaseConnection`の
 * 引数として必須項目のみをまとめた形にしている。
 */
export interface FirebaseConfig {
  apiKey: string;
  authDomain: string;
  projectId: string;
  appId: string;
}

/** `update_settings`コマンドへ渡す部分更新オブジェクト。 */
export type AppSettingsPatch = Partial<AppSettings>;
