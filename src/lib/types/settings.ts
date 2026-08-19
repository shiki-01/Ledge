/**
 * シェルフを表示する画面端。Rust側 `settings::ShelfEdge` と対応する。
 */
export type ShelfEdge = "top" | "bottom" | "left" | "right";

/**
 * アプリ設定。Rust側 `settings::AppSettings` とフィールド名を揃えている。
 * Phase1で実際に使うのはホットキー登録とシェルフの表示端のみで、
 * 表示位置/透明度設定UI自体（F-10）はPhase3で実装する。
 */
export interface AppSettings {
  shelfHotkey: string;
  shelfEdge: ShelfEdge;
  /** クリップボード履歴の自動クリア（F-16）: 件数上限。既定500件。設定画面UI自体はPhase3。 */
  clipboardMaxEntries: number;
  /** クリップボード履歴の自動クリア（F-16）: 経過日数上限。既定30日。設定画面UI自体はPhase3。 */
  clipboardRetentionDays: number;
}

/** `update_settings`コマンドへ渡す部分更新オブジェクト。 */
export type AppSettingsPatch = Partial<AppSettings>;
